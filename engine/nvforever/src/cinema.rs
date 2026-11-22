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
}

impl CinemaLook {
    /// Whether this draws anything.
    pub fn on(&self) -> bool {
        self.params.x > 0.0 || self.params.y > 0.0 || self.params.z > 0.0 || self.vignette.x > 0.0
    }
}

/// The camera's values for NVR's Cinema settings at time `seconds`.
pub fn look_for(c: &crate::nvr_look::Cinema, seconds: f32) -> CinemaLook {
    CinemaLook {
        params: Vec4::new(c.grain * GRAIN_PER_AMOUNT, c.aberration * ABERRATION_PER_UNIT, c.overlay, (seconds * GRAIN_FPS).floor() % 4096.0),
        vignette: Vec4::new(c.vignette_darkness, c.vignette_radius, 0.0, 0.0),
    }
}

pub struct CinemaPlugin;

impl Plugin for CinemaPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "cinema.wesl");
        app.add_plugins((ExtractComponentPlugin::<CinemaLook>::default(), UniformComponentPlugin::<CinemaLook>::default()))
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
fn follow(mut commands: Commands, look: Option<Res<NvrLook>>, time: Res<Time>, cams: Query<(Entity, Option<&CinemaLook>), With<FlyCam>>) {
    let cinema = look.as_deref().and_then(|l| l.cinema);
    for (e, has) in &cams {
        match cinema {
            Some(c) => {
                let new = look_for(&c, time.elapsed_secs_wrapped());
                if has.is_none_or(|h| h.params != new.params || h.vignette != new.vignette) {
                    commands.entity(e).insert(new);
                }
            }
            None if has.is_some() => {
                commands.entity(e).remove::<CinemaLook>();
            }
            None => {}
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
}
