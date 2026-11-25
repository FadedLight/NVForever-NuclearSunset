//! The engine's own take on New Vegas Reloaded's Cinema effects (film grain, chromatic
//! aberration, the image overlaid on itself, a vignette), with the strengths the load
//! order's NVR preset gives (`nvr_look.rs` [`crate::nvr_look::Cinema`]). Written from
//! scratch for this engine from what NVR's defaults file says each value does (no NVR
//! code); the formulas and scales are ours [engineering choice, by eye].
//!
//! One full-size pass at the very end of the main camera's post-processing (after the
//! image-space grade, so after any upscaler too: grain at the output's own pixels). The
//! interface is drawn by its own camera after all of this, so the HUD, menus and Pip-Boy
//! never pass through here, and emissive surfaces only get what every pixel gets.
//! Nothing is drawn without NVR's Cinema on, or with `NVF_NVR_LOOK=off`.
//!
//! The same pass draws NVR's Water Lens: for a few seconds after the camera comes up out
//! of the water, drops on the lens bend the picture behind them and run down, then fade
//! (`underwater.rs` [`crate::underwater::CameraSubmerged`] says when).

use crate::fly::FlyCam;
use crate::nvr_look::NvrLook;
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::core_pipeline::{Core3d, Core3dSystems, FullscreenShader};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer};
use bevy::render::render_resource::{
    AddressMode, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId, ColorTargetState, ColorWrites, FilterMode,
    FragmentState, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
    SamplerDescriptor, ShaderStages, ShaderType, TextureFormat, TextureSampleType,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedView, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

/// Grain amplitude (display value, ±) per unit of NVR's FilmGrainAmount [engineering
/// choice: 0.3 gives a fine ±1.8% grain, seen on flat sky, gone in detail].
const GRAIN_PER_AMOUNT: f32 = 0.06;
/// The grain's frame rate [observed: NVR's defaults file, "at 24fps like film"].
const GRAIN_FPS: f32 = 24.0;
/// Channel split at the screen's corners per unit of ChromaticAberration, as a share of
/// the picture's height [engineering choice: 1.0 is about 2 pixels at 1080p's corners,
/// none at the centre].
const ABERRATION_PER_UNIT: f32 = 0.0018;

/// Cinema on a camera: what [`follow`] takes from the NVR look each frame.
#[derive(Component, ExtractComponent, Clone, Copy, Default, ShaderType)]
#[extract_app(bevy::render::RenderApp)]
pub struct CinemaLook {
    /// x grain amplitude, y aberration (share of height at the corner), z overlay strength
    /// (0..1), w grain frame number.
    pub params: Vec4,
    /// x vignette darkness, y radius it starts at (share of the half-diagonal), zw unused.
    pub vignette: Vec4,
    /// Water drops on the lens after surfacing: x how much is left (1 just out of the
    /// water, 0 gone), y refraction (share of the picture's height), z how far the drops
    /// have run (share of the height), w a number for this surfacing (new drops each time).
    pub lens: Vec4,
}

impl CinemaLook {
    /// Whether this draws anything.
    pub fn on(&self) -> bool {
        self.params.x > 0.0 || self.params.y > 0.0 || self.params.z > 0.0 || self.vignette.x > 0.0 || self.lens.x > 0.0
    }
}

/// The camera's values for NVR's Cinema settings at time `seconds`.
pub fn look_for(c: &crate::nvr_look::Cinema, seconds: f32) -> CinemaLook {
    CinemaLook {
        params: Vec4::new(c.grain * GRAIN_PER_AMOUNT, c.aberration * ABERRATION_PER_UNIT, c.overlay, (seconds * GRAIN_FPS).floor() % 4096.0),
        vignette: Vec4::new(c.vignette_darkness, c.vignette_radius, 0.0, 0.0),
        lens: Vec4::ZERO,
    }
}

/// NVR's Water Lens "Time" read as frames at this rate [hypothesis: 180 makes the drops
/// last 3 s, about what a short lens effect wants].
const LENS_FRAMES_PER_SECOND: f32 = 60.0;
/// Refraction at the drops' edge per unit of NVR's Amount, as a share of the picture's
/// height [engineering choice: 0.1 bends the picture by about 2 pixels at 1080p].
const LENS_REFRACTION_PER_AMOUNT: f32 = 0.02;
/// How far the drops run per second per unit of the two speed factors, as a share of the
/// picture's height [engineering choice: NVR's 0.1 + 0.2 runs a drop about a tenth of the
/// screen down in its 3 s].
const LENS_RUN_PER_SECOND: f32 = 0.11;

/// The lens values `since` seconds after surfacing number `n` (`None`: under water or
/// never surfaced); all zero once the drops are gone.
pub fn lens_for(w: &crate::nvr_look::WaterLens, since: Option<f32>, n: u32) -> Vec4 {
    let Some(t) = since.filter(|t| *t >= 0.0) else { return Vec4::ZERO };
    let life = (w.time / LENS_FRAMES_PER_SECOND).max(0.1);
    let left = 1.0 - t / life;
    if left <= 0.0 || w.amount <= 0.0 {
        return Vec4::ZERO;
    }
    // Thicker water (higher viscosity) runs slower.
    let run = t * LENS_RUN_PER_SECOND * (w.mult_a + w.mult_b) / (1.0 + 10.0 * w.viscosity);
    Vec4::new(left, w.amount * LENS_REFRACTION_PER_AMOUNT, run, (n % 1024) as f32)
}

/// When the camera last came up out of the water, and how many times it has.
#[derive(Default)]
struct Surfacing {
    was_under: bool,
    at: Option<f32>,
    count: u32,
}

pub struct CinemaPlugin;

impl Plugin for CinemaPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "cinema.wesl");
        app.add_plugins((ExtractComponentPlugin::<CinemaLook>::default(), UniformComponentPlugin::<CinemaLook>::default()))
            .init_resource::<crate::underwater::CameraSubmerged>()
            .add_systems(PostUpdate, follow);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app.add_systems(RenderStartup, init_pipelines).add_systems(Render, prepare.in_set(RenderSystems::PrepareResources)).add_systems(
            Core3d,
            cinema_pass
                .in_set(Core3dSystems::PostProcess)
                .after(tonemapping)
                .after(crate::post::post_pass)
                .after(crate::imagespace::blur_pass)
                .after(bevy::core_pipeline::fullscreen_material::fullscreen_material_system::<crate::imagespace::ImageSpaceGrade>)
                .after(crate::shafts::shafts_pass)
                .after(crate::height_fog::height_fog_pass),
        );
    }
}

/// Put this frame's Cinema on the main camera (only there: a scope's lens camera keeps
/// its own picture, and the main camera's pass covers the lens on the screen).
fn follow(
    mut commands: Commands,
    look: Option<Res<NvrLook>>,
    time: Res<Time>,
    under: Option<Res<crate::underwater::CameraSubmerged>>,
    mut surfacing: Local<Surfacing>,
    cams: Query<(Entity, Option<&CinemaLook>), With<FlyCam>>,
) {
    let now = time.elapsed_secs();
    let is_under = under.is_some_and(|u| u.0);
    if surfacing.was_under && !is_under {
        surfacing.at = Some(now);
        surfacing.count = surfacing.count.wrapping_add(1);
        info!("water lens: out of the water, drops on the lens");
    }
    if is_under {
        surfacing.at = None;
    }
    surfacing.was_under = is_under;
    let cinema = look.as_deref().and_then(|l| l.cinema);
    let lens = look.as_deref().and_then(|l| l.water_lens).map_or(Vec4::ZERO, |w| lens_for(&w, surfacing.at.map(|at| now - at), surfacing.count));
    let mut new = cinema.map_or_else(CinemaLook::default, |c| look_for(&c, time.elapsed_secs_wrapped()));
    new.lens = lens;
    for (e, has) in &cams {
        if new.on() {
            if has.is_none_or(|h| h.params != new.params || h.vignette != new.vignette || h.lens != new.lens) {
                commands.entity(e).insert(new);
            }
        } else if has.is_some() {
            commands.entity(e).remove::<CinemaLook>();
        }
    }
}

#[derive(Resource)]
pub(crate) struct CinemaPipelines {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    base: RenderPipelineDescriptor,
    pipelines: HashMap<TextureFormat, CachedRenderPipelineId>,
}

#[derive(Component)]
pub(crate) struct CinemaView {
    pipeline: CachedRenderPipelineId,
    format: TextureFormat,
}

fn init_pipelines(mut commands: Commands, device: Res<RenderDevice>, assets: Res<AssetServer>, fullscreen: Res<FullscreenShader>) {
    let layout = BindGroupLayoutDescriptor::new(
        "cinema_layout",
        &BindGroupLayoutEntries::sequential(ShaderStages::FRAGMENT, (texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), uniform_buffer::<CinemaLook>(true))),
    );
    let sampler = device.create_sampler(&SamplerDescriptor {
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        ..default()
    });
    let base = RenderPipelineDescriptor {
        label: Some("cinema_pipeline".into()),
        layout: vec![],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState { shader: assets.load("embedded://nvforever/cinema.wesl"), entry_point: Some("composite".into()), ..default() }),
        ..default()
    };
    commands.insert_resource(CinemaPipelines { layout, sampler, base, pipelines: HashMap::default() });
}

fn prepare(mut commands: Commands, cache: Res<PipelineCache>, mut pipelines: ResMut<CinemaPipelines>, views: Query<(Entity, &ExtractedView), With<CinemaLook>>) {
    for (entity, view) in &views {
        let format = view.target_format;
        let pipeline = match pipelines.pipelines.get(&format) {
            Some(id) => *id,
            None => {
                let mut d = pipelines.base.clone();
                d.layout = vec![pipelines.layout.clone()];
                if let Some(f) = d.fragment.as_mut() {
                    f.targets = vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })];
                }
                let id = cache.queue_render_pipeline(d);
                pipelines.pipelines.insert(format, id);
                id
            }
        };
        commands.entity(entity).insert(CinemaView { pipeline, format });
    }
}

/// The Cinema effects over the picture, in one pass.
pub(crate) fn cinema_pass(
    view: ViewQuery<(&ViewTarget, &DynamicUniformIndex<CinemaLook>, &CinemaLook, &CinemaView)>,
    pipelines: Res<CinemaPipelines>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<CinemaLook>>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (target, index, look, cv) = view.into_inner();
    if !look.on() || !crate::ssao::format_matches(target, cv.format) {
        return;
    }
    let Some(pipeline) = cache.get_render_pipeline(cv.pipeline) else { return };
    let Some(binding) = uniforms.uniforms().binding() else { return };
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let span = diagnostics.time_span(ctx.command_encoder(), "nvf_cinema");
    let post = target.post_process_write();
    let layout = cache.get_bind_group_layout(&pipelines.layout);
    let bg = device.create_bind_group("cinema", &layout, &BindGroupEntries::sequential((post.source, &pipelines.sampler, binding)));
    {
        let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
            label: Some("cinema_pass"),
            color_attachments: &[Some(RenderPassColorAttachment { view: post.destination, depth_slice: None, resolve_target: None, ops: Operations::default() })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bg, &[index.index()]);
        pass.draw(0..3, 0..1);
    }
    span.end(ctx.command_encoder());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nvr_look::Cinema;

    #[test]
    fn hikikomoris_cinema_becomes_gentle_camera_values() {
        let c = Cinema { grain: 0.3, aberration: 1.0, overlay: 0.2, vignette_darkness: 0.0, vignette_radius: 0.0 };
        let l = look_for(&c, 1.0);
        assert!((l.params.x - 0.018).abs() < 1e-6, "±1.8% grain");
        assert!((l.params.y - ABERRATION_PER_UNIT).abs() < 1e-9);
        assert_eq!(l.params.z, 0.2);
        assert_eq!(l.params.w, 24.0, "24 grain frames a second");
        assert_eq!(look_for(&c, 1.02).params.w, 24.0, "the grain holds within a frame of film");
        assert_eq!(l.vignette.x, 0.0, "Hikikomori's vignette is off");
        assert!(l.on());
        let none = Cinema { grain: 0.0, aberration: 0.0, overlay: 0.0, vignette_darkness: 0.0, vignette_radius: 0.5 };
        assert!(!look_for(&none, 0.0).on());
    }

    #[test]
    fn water_drops_fade_after_surfacing() {
        let w = crate::nvr_look::WaterLens { amount: 0.1, time: 180.0, mult_a: 0.1, mult_b: 0.2, viscosity: 0.05 };
        assert_eq!(lens_for(&w, None, 1), Vec4::ZERO, "under water or never out of it");
        let start = lens_for(&w, Some(0.0), 1);
        assert_eq!(start.x, 1.0);
        assert!((start.y - 0.002).abs() < 1e-7);
        assert_eq!(start.z, 0.0);
        let mid = lens_for(&w, Some(1.5), 1);
        assert!((mid.x - 0.5).abs() < 1e-6, "180 frames: gone after 3 s");
        assert!(mid.z > 0.0, "the drops run down");
        assert_eq!(lens_for(&w, Some(3.0), 1), Vec4::ZERO);
        let mut c = CinemaLook::default();
        c.lens = mid;
        assert!(c.on(), "drops alone draw, without NVR's Cinema");
    }
}
