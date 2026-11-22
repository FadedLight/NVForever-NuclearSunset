//! Height fog, the engine's own take on what New Vegas Reloaded's Volumetric Fog adds to
//! Nuclear Sunset (results\REPORT-shaders-survey.md): haze that is thick low down and thins
//! with height, so valleys and the ground far off sink into the weather's fog colour while
//! the hills stand out, and that lights up towards the sun. Written from scratch for this
//! engine (no NVR code); NVR's settings only set how strong it is (`nvr_look.rs`
//! [`NvrLook::fog`]).
//!
//! One full-size pass on the tonemapped picture with the depth prepass, after the temporal
//! anti-aliasing and before the sun shafts, bloom and sharpening; outdoors only, never on
//! the sky, and only on the main camera while it carries [`HeightFog`]. The interface is
//! drawn after all of the camera's post-processing, so it never passes through here.
//!
//! The fog along a view ray: density `a·exp(−b·(h − h0))` (h height, h0 a little under the
//! eye) integrated in closed form from the eye to the surface; the surface keeps
//! `exp(−that)` of its colour and the rest is fog colour, brightened towards the sun.

use crate::fly::FlyCam;
use crate::nvr_look::NvrLook;
use crate::sky::SkyState;
use bevy::core_pipeline::prepass::ViewPrepassTextures;
use bevy::core_pipeline::tonemapping::tonemapping;
use bevy::core_pipeline::{Core3d, Core3dSystems, FullscreenShader};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::render::diagnostic::RecordDiagnostics;
use bevy::render::extract_component::{ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin, UniformComponentPlugin};
use bevy::render::render_resource::binding_types::{sampler, texture_2d, texture_depth_2d, texture_depth_2d_multisampled, uniform_buffer};
use bevy::render::render_resource::{
    AddressMode, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedRenderPipelineId, ColorTargetState, ColorWrites, FilterMode,
    FragmentState, Operations, PipelineCache, RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, Sampler, SamplerBindingType,
    SamplerDescriptor, ShaderStages, ShaderType, TextureFormat, TextureSampleType,
};
use bevy::render::renderer::{RenderContext, RenderDevice, ViewQuery};
use bevy::render::view::{ExtractedView, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

/// Optical depth along a level ray from the eye out to the weather's fog far distance, at
/// NVR's default Amount [engineering choice, by eye: Goodsprings at midday keeps its hills,
/// the far valley floor goes hazy].
const DEPTH_AT_FAR: f32 = 0.12;
/// How fast the fog thins with height: e-folding height, game units [engineering choice].
const THIN_OVER: f32 = 1500.0;
/// The fog's reference height under the eye, game units [engineering choice: the ground
/// under a standing player].
const BELOW_EYE: f32 = 300.0;
/// Lowest a ray's end may lie under the reference height in the density's exponent, game
/// units, so a deep canyon seen from a cliff does not go solid [engineering choice].
const DEEPEST: f32 = 3000.0;
/// How much the fog towards the sun takes of the sunlight [engineering choice].
const SUN_SCATTER: f32 = 0.35;

/// Height fog on a camera, kept up to date by [`follow`].
#[derive(Component, ExtractComponent, Clone, Copy, Default, ShaderType)]
#[extract_app(bevy::render::RenderApp)]
pub struct HeightFog {
    /// World from clip space (the view's inverse view-projection).
    pub world_from_clip: Mat4,
    /// xyz: the eye (world), w: the projection's near plane.
    pub eye: Vec4,
    /// rgb: fog colour (linear), w: density at the reference height (per world unit; 0 none).
    pub color: Vec4,
    /// x: 1 / e-folding height, y: reference height (world y), z: deepest below it,
    /// w: distance beyond which the depth is the sky dome's.
    pub shape: Vec4,
    /// xyz: towards the sun (world), w: sun scatter strength (0 at night).
    pub sun: Vec4,
    /// rgb: sunlight colour (linear).
    pub sun_color: Vec4,
}

pub struct HeightFogPlugin;

impl Plugin for HeightFogPlugin {
    fn build(&self, app: &mut App) {
        bevy::asset::embedded_asset!(app, "height_fog.wesl");
        app.add_plugins((ExtractComponentPlugin::<HeightFog>::default(), UniformComponentPlugin::<HeightFog>::default()))
            .add_systems(PostUpdate, follow.after(bevy::camera::CameraUpdateSystems).after(bevy::transform::TransformSystems::Propagate));
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app.add_systems(RenderStartup, init_pipelines).add_systems(Render, prepare.in_set(RenderSystems::PrepareResources)).add_systems(
            Core3d,
            height_fog_pass
                .in_set(Core3dSystems::PostProcess)
                .after(tonemapping)
                .after(crate::ssao::ao_pass)
                .after(crate::taa::taa_pass)
                .after(bevy::anti_alias::fxaa::fxaa)
                .after(bevy::anti_alias::smaa::smaa)
                .before(crate::shafts::shafts_pass)
                .before(crate::post::post_pass)
                .before(crate::imagespace::blur_pass),
        );
    }
}

fn display_to_linear(c: f32) -> f32 {
    c.max(0.0).powf(2.2)
}

/// The fog's density per world unit at the reference height for a weather whose fog far
/// distance is `far` (game units), its strength `strength` (0..1) and NVR's Amount ratio
/// `amount`; 0 without fog.
pub fn density(far: f32, strength: f32, amount: f32) -> f32 {
    if far <= 0.0 {
        return 0.0;
    }
    DEPTH_AT_FAR * amount.max(0.0) * strength.clamp(0.0, 1.0) / far
}

/// This frame's fog: the weather's fog colour and distance, NVR's strength, the sun.
fn follow(
    sky: Option<Res<SkyState>>,
    look: Option<Res<NvrLook>>,
    tweaks: Option<Res<nvf_play::tweaks::TweakSettings>>,
    mut cams: Query<(&GlobalTransform, &Projection, &mut HeightFog), With<FlyCam>>,
) {
    let Ok((gt, proj, mut fog)) = cams.single_mut() else { return };
    let Projection::Perspective(p) = proj else { return };
    let Some(sky) = sky else { return };
    let amount = look.as_deref().map_or(0.0, |l| l.fog);
    let mut a = 0.0;
    if sky.interior.is_none() {
        let range = sky.fog_override.map(|(n, f)| (n, f, 1.0)).or_else(|| sky.fog());
        // The Outdoor Fog Distance tweak spreads this fog as it does the game's.
        let spread = if sky.fog_override.is_some() { 1.0 } else { tweaks.as_deref().map_or(1.0, |t| t.outdoor_fog_distance).clamp(1.0, 4.0) };
        if let Some((_, far, strength)) = range {
            a = density(far * spread, strength, amount);
        }
    }
    let eye = gt.translation();
    let day = smoothstep(-2.0, 3.0, sky.sun_elevation());
    let fog_col = sky.color(fnv_world::sky::ColorType::Fog).map(display_to_linear);
    let sun_col = sky.color(fnv_world::sky::ColorType::Sunlight).map(display_to_linear);
    let world_from_clip = gt.to_matrix() * proj.get_clip_from_view().inverse();
    let new = HeightFog {
        world_from_clip,
        eye: eye.extend(p.near),
        color: Vec3::from_array(fog_col).extend(a),
        shape: Vec4::new(1.0 / THIN_OVER, eye.y - BELOW_EYE, DEEPEST, crate::sky::SKY_DEPTH_FROM),
        sun: sky.sun_dir().normalize_or_zero().extend(SUN_SCATTER * day),
        sun_color: Vec3::from_array(sun_col).extend(0.0),
    };
    if fog.world_from_clip != new.world_from_clip || fog.color != new.color || fog.sun != new.sun || fog.shape != new.shape || fog.sun_color != new.sun_color {
        *fog = new;
    }
}

fn smoothstep(a: f32, b: f32, x: f32) -> f32 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[derive(Resource)]
pub(crate) struct HeightFogPipelines {
    /// With a single-sampled and a multisampled depth.
    layouts: [BindGroupLayoutDescriptor; 2],
    sampler: Sampler,
    base: RenderPipelineDescriptor,
    pipelines: HashMap<(bool, TextureFormat), CachedRenderPipelineId>,
}

#[derive(Component)]
pub(crate) struct HeightFogView {
    pipeline: CachedRenderPipelineId,
    multi: bool,
    format: TextureFormat,
}

fn init_pipelines(mut commands: Commands, device: Res<RenderDevice>, assets: Res<AssetServer>, fullscreen: Res<FullscreenShader>) {
    let layout = |multi: bool| {
        let depth = if multi { texture_depth_2d_multisampled() } else { texture_depth_2d() };
        BindGroupLayoutDescriptor::new(
            "height_fog_layout",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (texture_2d(TextureSampleType::Float { filterable: true }), sampler(SamplerBindingType::Filtering), uniform_buffer::<HeightFog>(true), depth),
            ),
        )
    };
    let sampler = device.create_sampler(&SamplerDescriptor {
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        ..default()
    });
    let base = RenderPipelineDescriptor {
        label: Some("height_fog_pipeline".into()),
        layout: vec![],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState { shader: assets.load("embedded://nvforever/height_fog.wesl"), entry_point: Some("composite".into()), ..default() }),
        ..default()
    };
    commands.insert_resource(HeightFogPipelines { layouts: [layout(false), layout(true)], sampler, base, pipelines: HashMap::default() });
}

fn prepare(mut commands: Commands, cache: Res<PipelineCache>, mut pipelines: ResMut<HeightFogPipelines>, views: Query<(Entity, &ExtractedView, &Msaa), With<HeightFog>>) {
    for (entity, view, msaa) in &views {
        let multi = msaa.samples() > 1;
        let format = view.target_format;
        let pipeline = match pipelines.pipelines.get(&(multi, format)) {
            Some(id) => *id,
            None => {
                let mut d = pipelines.base.clone();
                d.layout = vec![pipelines.layouts[multi as usize].clone()];
                if let Some(f) = d.fragment.as_mut() {
                    f.targets = vec![Some(ColorTargetState { format, blend: None, write_mask: ColorWrites::ALL })];
                    if multi {
                        f.shader_defs.push("MULTISAMPLED".into());
                    }
                }
                let id = cache.queue_render_pipeline(d);
                pipelines.pipelines.insert((multi, format), id);
                id
            }
        };
        commands.entity(entity).insert(HeightFogView { pipeline, multi, format });
    }
}

/// The fog over the picture, in one pass.
pub(crate) fn height_fog_pass(
    view: ViewQuery<(&ViewTarget, &DynamicUniformIndex<HeightFog>, &HeightFog, &HeightFogView, &ViewPrepassTextures)>,
    pipelines: Res<HeightFogPipelines>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<HeightFog>>,
    device: Res<RenderDevice>,
    mut ctx: RenderContext,
) {
    let (target, index, fog, fv, prepass) = view.into_inner();
    if fog.color.w <= 0.0 {
        return;
    }
    if crate::ssao::depth_multisampled(prepass) != Some(fv.multi) || !crate::ssao::format_matches(target, fv.format) {
        return;
    }
    let Some(depth) = prepass.depth_only_view() else { return };
    let Some(pipeline) = cache.get_render_pipeline(fv.pipeline) else { return };
    let Some(binding) = uniforms.uniforms().binding() else { return };
    let diagnostics = ctx.diagnostic_recorder();
    let diagnostics = diagnostics.as_deref();
    let span = diagnostics.time_span(ctx.command_encoder(), "nvf_height_fog");
    let post = target.post_process_write();
    let layout = cache.get_bind_group_layout(&pipelines.layouts[fv.multi as usize]);
    let bg = device.create_bind_group("height_fog", &layout, &BindGroupEntries::sequential((post.source, &pipelines.sampler, binding, depth)));
    {
        let mut pass = ctx.command_encoder().begin_render_pass(&RenderPassDescriptor {
            label: Some("height_fog_pass"),
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

    #[test]
    fn fog_density_follows_the_weather_and_nvrs_amount() {
        assert_eq!(density(0.0, 1.0, 1.0), 0.0, "no fog distance, no fog");
        assert_eq!(density(100_000.0, 0.0, 1.0), 0.0, "a weather fade with no fog");
        assert_eq!(density(100_000.0, 1.0, 0.0), 0.0, "NVR's fog off");
        // Optical depth over the fog far distance at the reference height.
        assert!((density(100_000.0, 1.0, 1.0) * 100_000.0 - DEPTH_AT_FAR).abs() < 1e-6);
        assert!((density(50_000.0, 1.0, 2.0) * 50_000.0 - 2.0 * DEPTH_AT_FAR).abs() < 1e-6);
    }}
