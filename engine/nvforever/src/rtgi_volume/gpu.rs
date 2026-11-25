//! The render world's side: what plan.rs decided, carried out. The component is inserted, changed
//! and removed in the extraction alone, as bevy_pbr requires (mesh_view_bindings.rs).

use super::plan::{plan, Admission, Key, Latch, Now, Pipelines, Plan, Why};
use super::{hook_state, place, Did, Place, RtgiCounts, RtgiRequest, RtgiVolumeUniform};
use bevy::app::SubApp;
use bevy::ecs::system::SystemParam;
use bevy::pbr::NvfRtgiVolumeBindings;
use bevy::prelude::*;
use bevy::render::render_resource::{
    AddressMode, CachedPipelineState, Extent3d, FilterMode, PipelineCache, PipelineDescriptor, Sampler, SamplerDescriptor, TextureDataOrder, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView, TextureViewDescriptor, UniformBuffer,
};
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::sync_world::RenderEntity;
use bevy::render::{Extract, ExtractSchedule, GpuResourceAppExt, Render, RenderSystems};
use nvf_assets::nif_material::RTGI_VOLUME_DEF;

pub(super) fn add(render_app: &mut SubApp) {
    render_app
        .init_gpu_resource::<RtgiGpu>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(Render, observe.in_set(RenderSystems::Cleanup));
}

/// What is bound.
pub(super) struct Held {
    pub(super) key: Key,
    buffer: UniformBuffer<RtgiVolumeUniform>,
    written: RtgiVolumeUniform,
    texture: TextureView,
    sampler: Sampler,
}

#[derive(Resource, Default)]
pub(super) struct RtgiGpu {
    pub(super) held: Option<Held>,
    latch: Latch,
    /// The visit in which a pipeline that reads failed.
    failed: Option<Place>,
    /// The visit whose pipelines the log has, and the visit of which it says why the light is stock.
    told_ready: Option<Place>,
    told_stock: Option<(Place, Why)>,
}

/// The main world's, at the extraction.
#[derive(SystemParam)]
struct Main<'w, 's> {
    request: Option<Res<'w, RtgiRequest>>,
    streamer: Option<Res<'w, crate::world::Streamer>>,
    script: Option<Res<'w, nvf_play::script::ScriptState>>,
    loading: Option<Res<'w, nvf_play::loading::Loading>>,
    cams: Query<'w, 's, RenderEntity, crate::fly::WorldCamera>,
}

/// The device.
#[derive(SystemParam)]
struct Device<'w> {
    device: Option<Res<'w, RenderDevice>>,
    queue: Option<Res<'w, RenderQueue>>,
}

impl Device<'_> {
    /// Nothing is asked of it yet but that it is there.
    fn admission(&self) -> Admission {
        if self.device.is_some() && self.queue.is_some() {
            Admission::Granted
        } else {
            Admission::Deferred
        }
    }
}

fn extract(mut commands: Commands, main: Extract<Main>, mut bound: Query<(Entity, &mut NvfRtgiVolumeBindings)>, mut state: ResMut<RtgiGpu>, gpu: Device, counts: Res<RtgiCounts>) {
    let state = &mut *state;
    let loading = main.loading.as_deref();
    let open = state.latch.step(loading.is_some_and(|l| l.starting() || l.travelling()), loading.is_some_and(|l| l.settling()));
    let request = main.request.as_deref();
    if request.is_none() && state.held.is_none() && bound.is_empty() {
        return;
    }
    let here = place(main.streamer.as_deref(), main.script.as_deref());
    let admission = match request.filter(|r| here == Some(r.place)) {
        Some(_) => gpu.admission(),
        None => Admission::Deferred,
    };
    let now = Now { here, hook: hook_state(), admission, open, failed: state.failed };
    let plan = plan(request.map(RtgiRequest::key), &now, state.held.as_ref().map(|h| h.key));
    tell(state, request, plan);
    let cams: Vec<Entity> = main.cams.iter().collect();
    carry_out(plan, request, &cams, &mut commands, &mut bound, &mut state.held, gpu.device.as_deref().zip(gpu.queue.as_deref()), &counts);
}

/// The log's line of a change.
fn tell(state: &mut RtgiGpu, request: Option<&RtgiRequest>, plan: Plan) {
    match (plan, request) {
        (Plan::Bind { new: true }, Some(r)) => {
            let what = if r.solved.is_some() { "a solved texture" } else { "the empty texture" };
            info!("rtgi: cell {:08X}, revision {}: {what} of {:?} bound", r.place.cell, r.revision, r.atlas().0);
        }
        (Plan::Stock(why), _) => {
            if state.held.is_some() {
                info!("rtgi: unbound ({why:?}), the stock light");
            }
            // The log has a refusal and a failed pipeline already.
            let Some(r) = request.filter(|r| matches!(why, Why::Hook | Why::Late) && state.told_stock != Some((r.place, why))) else { return };
            state.told_stock = Some((r.place, why));
            if why == Why::Hook {
                error!("rtgi: the hook is not in the lighting; the stock light");
            } else {
                info!("rtgi: cell {:08X}: asked for with no screen up that waits for its pipelines; the stock light until the next visit", r.place.cell);
            }
        }
        _ => {}
    }
}

/// `gpu` is needed for [`Plan::Bind`] alone, which is never planned without (it is admitted first).
#[allow(clippy::too_many_arguments)]
pub(super) fn carry_out(
    plan: Plan,
    request: Option<&RtgiRequest>,
    cams: &[Entity],
    commands: &mut Commands,
    bound: &mut Query<(Entity, &mut NvfRtgiVolumeBindings)>,
    held: &mut Option<Held>,
    gpu: Option<(&RenderDevice, &RenderQueue)>,
    counts: &RtgiCounts,
) {
    match (plan, request) {
        (Plan::Bind { new }, Some(request)) => {
            let (device, queue) = gpu.expect("a device that granted it");
            let uniform = request.uniform();
            if new {
                // The GPU lets go of what was held when it is done with it; the buffer and the
                // sampler stay.
                let texture = make_texture(device, queue, request.atlas());
                counts.add(Did::Allocation, 1);
                counts.add(Did::Upload, 1);
                match held {
                    Some(h) => (h.key, h.texture) = (request.key(), texture),
                    None => {
                        let mut buffer = UniformBuffer::from(uniform);
                        buffer.set_label(Some("nvf_rtgi_volume"));
                        buffer.write_buffer(device, queue);
                        counts.add(Did::Allocation, 2);
                        counts.add(Did::Upload, 1);
                        *held = Some(Held { key: request.key(), buffer, written: uniform, texture, sampler: make_sampler(device) });
                    }
                }
            }
            if let Some(h) = held.as_mut().filter(|h| h.written != uniform) {
                h.buffer.set(uniform);
                h.buffer.write_buffer(device, queue);
                h.written = uniform;
                counts.add(Did::Upload, 1);
            }
        }
        (Plan::Keep, _) => {}
        _ => *held = None,
    }
    let shown = held.as_ref().and_then(|h| Some((h, h.buffer.buffer()?)));
    for (e, mut b) in bound.iter_mut() {
        match shown.filter(|_| cams.contains(&e)) {
            Some((h, buffer)) => {
                if b.texture.id() != h.texture.id() || b.uniform.id() != buffer.id() {
                    *b = NvfRtgiVolumeBindings { uniform: buffer.clone(), texture: h.texture.clone(), sampler: h.sampler.clone() };
                    counts.add(Did::Bind, 1);
                }
            }
            None => {
                commands.entity(e).try_remove::<NvfRtgiVolumeBindings>();
            }
        }
    }
    let Some((h, buffer)) = shown else { return };
    for e in cams.iter().filter(|e| !bound.contains(**e)) {
        commands.entity(*e).try_insert(NvfRtgiVolumeBindings { uniform: buffer.clone(), texture: h.texture.clone(), sampler: h.sampler.clone() });
        counts.add(Did::Bind, 1);
    }
}

fn make_texture(device: &RenderDevice, queue: &RenderQueue, (sides, bytes): ([u32; 3], &[u8])) -> TextureView {
    let [width, height, depth_or_array_layers] = sides;
    let texture = device.create_texture_with_data(
        queue,
        &TextureDescriptor {
            label: Some("nvf_rtgi_volume"),
            size: Extent3d { width, height, depth_or_array_layers },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D3,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        },
        TextureDataOrder::LayerMajor,
        bytes,
    );
    texture.create_view(&TextureViewDescriptor::default())
}

fn make_sampler(device: &RenderDevice) -> Sampler {
    device.create_sampler(&SamplerDescriptor {
        label: Some("nvf_rtgi_volume"),
        address_mode_u: AddressMode::ClampToEdge,
        address_mode_v: AddressMode::ClampToEdge,
        address_mode_w: AddressMode::ClampToEdge,
        mag_filter: FilterMode::Linear,
        min_filter: FilterMode::Linear,
        ..default()
    })
}

/// Whether a pipeline's fragment reads the volume, by its def.
fn reads(descriptor: &PipelineDescriptor) -> bool {
    let PipelineDescriptor::RenderPipelineDescriptor(d) = descriptor else { return false };
    d.fragment.as_ref().is_some_and(|f| f.shader_defs.contains(&RTGI_VOLUME_DEF.into()))
}

/// The pipelines that read, once a frame while a texture is bound: one that failed for good ends the
/// visit's light (the next extraction takes the component away, before anything is specialised). It
/// is told by the def alone, so one whose stock twin is broken too counts as well [design].
fn observe(cache: Res<PipelineCache>, mut state: ResMut<RtgiGpu>) {
    let Some(place) = state.held.as_ref().map(|h| h.key.place).filter(|p| state.failed != Some(*p)) else { return };
    let mut n = Pipelines::default();
    for p in cache.pipelines().filter(|p| reads(&p.descriptor)) {
        n.count(&p.state);
        if let CachedPipelineState::Err(e) = &p.state {
            debug!("rtgi: a pipeline that reads the volume: {e}");
        }
    }
    if n.failed > 0 {
        state.failed = Some(place);
        error!("rtgi: {} pipelines that read the volume failed ({} are there); the stock light in this visit", n.failed, n.there);
    } else if n.there > 0 && n.waiting == 0 && state.told_ready != Some(place) {
        state.told_ready = Some(place);
        info!("rtgi: pipelines that read the volume: {} there, 0 to come, 0 failed", n.there);
    }
}
