//! The developer aid `NVF_PLAY_DEBUG_RTGI_MADE=1` (builds with the aids, `crate::dev_aids`): a
//! volume made by hand, not solved, bound the way a solved one is in every interior, and the read
//! of it on the GPU compared once with the CPU's (`nvf_rtgi::Reader`) [design].
//!
//! While the screen of a start or a travel is up the view has the empty texture, so its pipelines
//! are the ones that read; afterwards the made one takes its place (the same key). It stands around
//! the camera, and everywhere else the light is the one without support (rtgi-volume.md): what is
//! seen means nothing.

use super::gpu::RtgiGpu;
use super::{place, Did, Fit, Place, RtgiCounts, RtgiRequest, RtgiVolumeUniform};
use bevy::asset::{load_internal_asset, uuid_handle};
use bevy::pbr::NvfRtgiVolumeBindings;
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{sampler, storage_buffer_read_only_sized, storage_buffer_sized, texture_3d, uniform_buffer};
use bevy::render::render_resource::{
    BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, CachedComputePipelineId, ComputePassDescriptor, ComputePipelineDescriptor, PipelineCache, SamplerBindingType, ShaderStages, TextureSampleType,
};
use bevy::render::renderer::{RenderContext, RenderDevice, RenderGraph, RenderGraphSystems};
use bevy::render::storage::{GpuShaderBuffer, ShaderBuffer};
use bevy::render::RenderApp;
use bevy::shader::Shader;
use nvf_rtgi::{Counts, Read, Reader, SolvedVolume, SLABS, V3};
use std::sync::Arc;

/// rtgi_readback.wesl.
const SHADER: Handle<Shader> = uuid_handle!("f5b7a1c3-2e64-4d0e-9a38-61c0f7d25b12");

pub(super) fn wanted() -> bool {
    crate::dev_aids::var("NVF_PLAY_DEBUG_RTGI_MADE").as_deref() == Some("1")
}

// ---------------------------------------------------------------------------------------
// The volume.

const POINTS: [u32; 3] = [3, 2, 4];
const SPACING: f32 = 64.0;
const OFFSET: f32 = 24.0;
/// The light's constants of the aid alone.
pub(super) const FIT: Fit = Fit { b: 0.25, g: 1.0, pay: 1.0, least: 0.25, pull: 0.25 };
/// The points that do not count, and one that does and is black.
pub(super) const UNSUPPORTED: [[u32; 3]; 3] = [[1, 0, 0], [0, 1, 2], [2, 1, 3]];
const BLACK: [u32; 3] = [2, 0, 1];

/// C0, Cx, Cy or Cz (0 to 3) of a point: another at every point, in every channel and slab, which
/// half floats hold as they are; the last three of either sign.
fn value(slab: usize, [x, y, z]: [u32; 3], k: usize) -> f32 {
    let k = k as u32;
    if slab == 0 {
        return (16 + 5 * x + 9 * y + 11 * z + 7 * k) as f32 / 64.0;
    }
    // Steps of a prime 29, which the 24 points do not go through.
    let step = [0, 7, 11, 13][slab];
    (((x + POINTS[0] * (y + POINTS[1] * z)) * step + 9 * k) % 29) as f32 / 32.0 - 0.4375
}

/// The four texels of a point.
pub(super) fn texels(at: [u32; 3]) -> [[f32; 4]; SLABS] {
    if UNSUPPORTED.contains(&at) {
        return [[0.0; 4]; SLABS];
    }
    let c: [[f32; 3]; SLABS] = std::array::from_fn(|slab| std::array::from_fn(|k| if at == BLACK { 0.0 } else { value(slab, at, k) }));
    let alpha = [1.0, Vec3::from(c[0]).dot(Vec3::new(0.2126, 0.7152, 0.0722)), 0.0, 0.0];
    std::array::from_fn(|slab| Vec3::from(c[slab]).extend(alpha[slab]).to_array())
}

pub(super) fn points() -> impl Iterator<Item = [u32; 3]> {
    (0..POINTS.iter().product()).map(|i: u32| [i % POINTS[0], i / POINTS[0] % POINTS[1], i / (POINTS[0] * POINTS[1])])
}

/// The volume whose first point is at `low` times the spacing: 5 x 4 x 24 texels, a row of 40 bytes.
pub(super) fn made(low: [i32; 3]) -> SolvedVolume {
    let atlas = [POINTS[0] + 2, POINTS[1] + 2, SLABS as u32 * (POINTS[2] + 2)];
    let mut rgba16f_le = vec![0u8; 8 * (atlas[0] * atlas[1] * atlas[2]) as usize];
    for at in points() {
        for (slab, texel) in texels(at).into_iter().enumerate() {
            let to = texel_at(atlas, POINTS[2], at.map(|c| c as usize + 1), slab);
            for (k, c) in texel.into_iter().enumerate() {
                rgba16f_le[to + 2 * k..to + 2 * k + 2].copy_from_slice(&half::f16::from_f32(c).to_le_bytes());
            }
        }
    }
    let (points, unknown) = (points().count(), UNSUPPORTED.len());
    SolvedVolume {
        low,
        origin: low.map(|c| c as f32 * SPACING),
        spacing: SPACING,
        offset: OFFSET,
        emission: 0.0,
        dimensions: POINTS,
        atlas,
        rgba16f_le,
        counts: Counts { points, reached: points - unknown, unknown, ..default() },
        coverage: default(),
        stats: default(),
    }
}

/// Where the bytes of a slab's texel are, by its place in the padded slab.
fn texel_at(atlas: [u32; 3], deep: u32, at: [usize; 3], slab: usize) -> usize {
    let z = at[2] + slab * (deep as usize + 2);
    8 * ((z * atlas[1] as usize + at[1]) * atlas[0] as usize + at[0])
}

/// A slab's texel as it is stored.
fn stored(v: &SolvedVolume, at: [usize; 3], slab: usize) -> [f32; 4] {
    let from = texel_at(v.atlas, v.dimensions[2], at, slab);
    std::array::from_fn(|k| half::f16::from_le_bytes([v.rgba16f_le[from + 2 * k], v.rgba16f_le[from + 2 * k + 1]]).to_f32())
}

// ---------------------------------------------------------------------------------------
// The reads.

/// What a read is of.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// At a texel's centre: nothing between, so the right texel of every slab or not. Also outside.
    Centre,
    /// Between the points.
    Fraction,
    /// Less support than is taken as all there is.
    Thin,
    /// In the shell about the points.
    Shell,
}

pub(super) const KINDS: [(Kind, &str); 4] = [(Kind::Centre, "centre"), (Kind::Fraction, "fraction"), (Kind::Thin, "thin"), (Kind::Shell, "shell")];

/// A read, and what the CPU has of it.
#[derive(Clone, Copy, Debug)]
pub(super) struct Check {
    pub(super) kind: Kind,
    pub(super) p: [f32; 3],
    pub(super) g: [f32; 3],
    pub(super) n: [f32; 3],
    /// E and v.
    pub(super) want: [f32; 4],
    /// How far the GPU's may be.
    pub(super) bound: [f32; 4],
}

pub(super) const READS: usize = 96;

/// The filter's error that is let through, in texels on an axis [design]: 8 fractional bits and what
/// the coordinate's own arithmetic loses. A threshold on the machines the smoke test runs on; it says
/// nothing of a GPU's precision.
const EPS: f32 = 1.0 / 256.0 + 1e-4;

/// The error of arithmetic alone that is let through.
fn arithmetic(value: f32) -> f32 {
    1e-4 + value.abs() / 2048.0
}

/// How far a filtered channel of the cell whose low corner is `low` may be, for an error of [`EPS`]
/// on each axis: that times the largest difference of two corners next to each other, axis by axis.
fn delta(v: &SolvedVolume, low: [usize; 3], slab: usize, channel: usize) -> f32 {
    let corner = |i: usize| stored(v, std::array::from_fn(|k| low[k] + ((i >> k) & 1)), slab)[channel];
    let range = |axis: usize| (0..8).filter(|i| (i >> axis) & 1 == 0).map(|i| (corner(i | (1 << axis)) - corner(i)).abs()).fold(0.0, f32::max);
    EPS * (range(0) + range(1) + range(2))
}

/// How far a read at `q` (in spacings from the first point) may be from the CPU's: with a numerator's
/// error N and a denominator d of the error D, |dE| <= (N + |E| D) / (d - D).
fn filtered(v: &SolvedVolume, q: [f32; 3], n: [f32; 3], read: &Read) -> [f32; 4] {
    let low = q.map(|q| (q + 1.0).floor() as usize);
    let (d, dd) = (read.support.max(nvf_rtgi::THIN), delta(v, low, 0, 3));
    let over = |error: f32, value: f32| (error + value.abs() * dd) / (d - dd);
    let e = |k: usize| {
        let bands: f32 = (0..3).map(|j| n[j].abs() * delta(v, low, j + 1, k)).sum();
        over(delta(v, low, 0, k) + bands, read.raw.0[k])
    };
    [e(0), e(1), e(2), over(delta(v, low, 1, 3), read.v())]
}

/// The geometric normals and the shading normals the reads go through.
const G: [[f32; 3]; 6] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, -1.0]];
const N: [[f32; 3]; 8] =
    [[0.48, 0.6, 0.64], [-0.48, 0.64, 0.6], [0.64, -0.6, 0.48], [0.6, 0.48, -0.64], [-0.64, -0.48, 0.6], [0.36, 0.48, 0.8], [-0.6, 0.64, -0.48], [0.8, -0.36, -0.48]];
/// Parts of a spacing, none a whole number of 256ths.
const PARTS: [f32; 5] = [0.1873, 0.3021, 0.4547, 0.6138, 0.7711];

/// The places of the reads in spacings from the first point, each with its kind and shading normal.
fn places() -> Vec<(Kind, [f32; 3], [f32; 3])> {
    let mut out = Vec::with_capacity(READS);
    let part = |i: usize| PARTS[i % PARTS.len()];
    let last = POINTS.map(|d| d as f32 - 1.0);
    // Every point, and of four of them every slab alone.
    out.extend(points().enumerate().map(|(i, at)| (Kind::Centre, at.map(|c| c as f32), N[i % N.len()])));
    for at in [[0.0, 0.0, 0.0], [2.0, 1.0, 1.0], [1.0, 1.0, 3.0], [1.0, 0.0, 2.0]] {
        out.extend([[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].map(|n| (Kind::Centre, at, n)));
    }
    // The shell's own texels are outside: nothing.
    out.extend((0..8).map(|i| {
        let mut q = [1.0, 1.0, 2.0];
        q[i % 3] = if i % 2 == 0 { -1.0 } else { last[i % 3] + 1.0 };
        (Kind::Centre, q, N[i])
    }));
    out.extend((0..24).map(|i| (Kind::Fraction, [(i % 2) as f32 + part(i), part(i + 2), (i % 3) as f32 + part(i + 3)], N[i % N.len()])));
    // Far out on two sides of the shell: a part of less than a tenth.
    out.extend((0..12).map(|i| {
        let x = if i % 2 == 0 { -0.6138 } else { last[0] + 0.6138 };
        let y = if i / 2 % 2 == 0 { -0.7711 } else { last[1] + 0.7711 };
        (Kind::Thin, [x, y, (i % 3) as f32 + part(i)], N[i % N.len()])
    }));
    // Out on one side.
    out.extend((0..12).map(|i| {
        let mut q = [(i % 2) as f32 + part(i), part(i + 1), (i % 3) as f32 + part(i + 2)];
        let by = 0.3021 + 0.1 * (i / 6) as f32;
        q[i % 3] = if i % 2 == 0 { -by } else { last[i % 3] + by };
        (Kind::Shell, q, N[i % N.len()])
    }));
    out
}

/// The reads of a volume of [`POINTS`], as the CPU has them.
pub(super) fn checks(v: &SolvedVolume) -> Result<Vec<Check>, String> {
    let reader = Reader::new(v)?;
    let check = |(i, (kind, q, n)): (usize, (Kind, [f32; 3], [f32; 3]))| {
        let g = G[i % G.len()];
        let p: [f32; 3] = std::array::from_fn(|k| v.origin[k] + v.spacing * q[k] - v.offset * g[k]);
        let read = reader.read(V3(p), V3(g), V3(n));
        let want = Vec3::from(read.e().0).extend(read.v()).to_array();
        let mut bound = want.map(arithmetic);
        if kind != Kind::Centre {
            // No less than a centre's: there is arithmetic where the corners are alike too.
            let filtered = filtered(v, q, n, &read);
            bound = std::array::from_fn(|k| bound[k].max(filtered[k]));
        }
        Check { kind, p, g, n, want, bound }
    };
    Ok(places().into_iter().enumerate().map(check).collect())
}

/// The largest difference over its bound, of each of [`KINDS`]; none before the pass has run.
pub(super) fn judge(answers: &[[f32; 4]], checks: &[Check]) -> Option<[f32; 4]> {
    let (found, done) = answers.split_at_checked(checks.len())?;
    if done.first()?[0] != 1.0 {
        return None;
    }
    Some(KINDS.map(|(kind, _)| {
        let of = found.iter().zip(checks).filter(|(_, c)| c.kind == kind);
        let over = of.flat_map(|(found, c)| (0..4).map(move |k| (found[k] - c.want[k]).abs() / c.bound[k]));
        // What is no number is as far off as can be.
        over.fold(0.0, |worst, o| if o.is_nan() { f32::INFINITY } else { o.max(worst) })
    }))
}

// ---------------------------------------------------------------------------------------
// The aid.

pub(super) struct MadePlugin;

impl Plugin for MadePlugin {
    fn build(&self, app: &mut App) {
        info!("rtgi: NVF_PLAY_DEBUG_RTGI_MADE: a volume made by hand in every interior, its read checked once on the GPU");
        load_internal_asset!(app, SHADER, "../rtgi_readback.wesl", |s, _| Shader::from_wesl(s, "embedded://nvforever/rtgi_readback.wesl"));
        app.add_plugins(ExtractResourcePlugin::<Job>::default()).add_systems(PostUpdate, ask);
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return };
        render_app.init_resource::<Pass>().add_systems(RenderGraph, dispatch.in_set(RenderGraphSystems::Begin));
    }
}

/// The readback's buffers.
#[derive(Resource, Clone, ExtractResource)]
#[extract_app(RenderApp)]
struct Job {
    asked: Handle<ShaderBuffer>,
    answers: Handle<ShaderBuffer>,
}

/// The request of the place the player is in: the empty texture, and the made one once no screen is up.
#[allow(clippy::too_many_arguments)]
fn ask(
    mut commands: Commands,
    request: Option<Res<RtgiRequest>>,
    streamer: Option<Res<crate::world::Streamer>>,
    script: Option<Res<nvf_play::script::ScriptState>>,
    loading: Option<Res<nvf_play::loading::Loading>>,
    camera: Query<&GlobalTransform, crate::fly::WorldCamera>,
    job: Option<Res<Job>>,
    mut buffers: ResMut<Assets<ShaderBuffer>>,
) {
    let Some(here) = place(streamer.as_deref(), script.as_deref()) else {
        if request.is_some() {
            commands.remove_resource::<RtgiRequest>();
        }
        return;
    };
    let Some(request) = request.filter(|r| r.place == here) else {
        commands.insert_resource(RtgiRequest::empty(here, 1));
        return;
    };
    if request.solved.is_some() || loading.is_some_and(|l| l.active()) {
        return;
    }
    let Ok(camera) = camera.single() else { return };
    match show(here, camera.translation()) {
        Ok((request, checks)) => {
            commands.insert_resource(request);
            if job.is_none() {
                read_back(&mut commands, &mut buffers, checks);
            }
        }
        Err(why) => error!("rtgi: the made volume: {why}"),
    }
}

/// The made volume about a place, and its reads.
fn show(here: Place, about: Vec3) -> Result<(RtgiRequest, Vec<Check>), String> {
    let low = std::array::from_fn(|k| (about[k] / SPACING).round() as i32 - POINTS[k] as i32 / 2);
    let volume = made(low);
    let checks = checks(&volume)?;
    Ok((RtgiRequest::solved(here, 1, Arc::new(volume), FIT, 1.0)?, checks))
}

/// Ask the GPU the same, once, and say what it has.
fn read_back(commands: &mut Commands, buffers: &mut Assets<ShaderBuffer>, checks: Vec<Check>) {
    let asked: Vec<[f32; 4]> = checks.iter().flat_map(|c| [c.p, c.g, c.n]).map(|[x, y, z]| [x, y, z, 0.0]).collect();
    let answers = ShaderBuffer::from(vec![[0.0f32; 4]; checks.len() + 1]);
    let job = Job { asked: buffers.add(ShaderBuffer::from(asked)), answers: buffers.add(answers) };
    commands.spawn((Name::new("RTGI readback"), Readback::buffer(job.answers.clone()))).observe(move |read: On<ReadbackComplete>, mut commands: Commands| {
        let floats: Vec<f32> = read.data.as_chunks::<4>().0.iter().map(|b| f32::from_le_bytes(*b)).collect();
        let Some(worst) = judge(floats.as_chunks::<4>().0, &checks) else { return };
        commands.entity(read.entity).try_despawn();
        let most = (0..KINDS.len()).fold(0, |m, i| if worst[i] > worst[m] { i } else { m });
        let of: Vec<String> = KINDS.iter().zip(worst).map(|((_, name), w)| format!("{name} {w:.3}")).collect();
        info!("rtgi: readback: {} reads, the largest difference is {:.3} of its bound ({}); {}", checks.len(), worst[most], KINDS[most].1, of.join(", "));
    });
    commands.insert_resource(job);
}

/// The compute pass, and whether it has run.
#[derive(Resource, Default)]
struct Pass {
    pipeline: Option<CachedComputePipelineId>,
    done: bool,
}

/// Group 0 as the view has it, for the compute stage; group 1 the reads and the answers.
pub(super) fn layouts() -> [BindGroupLayoutDescriptor; 2] {
    let stage = ShaderStages::COMPUTE;
    let filterable = TextureSampleType::Float { filterable: true };
    [
        BindGroupLayoutDescriptor::new(
            "rtgi_readback_volume",
            &BindGroupLayoutEntries::with_indices(stage, ((47, uniform_buffer::<RtgiVolumeUniform>(false)), (48, texture_3d(filterable)), (49, sampler(SamplerBindingType::Filtering)))),
        ),
        BindGroupLayoutDescriptor::new("rtgi_readback_reads", &BindGroupLayoutEntries::sequential(stage, (storage_buffer_read_only_sized(false, None), storage_buffer_sized(false, None)))),
    ]
}

/// Once, from what a view has bound, when that is the made texture.
fn dispatch(
    job: Option<Res<Job>>,
    mut pass: ResMut<Pass>,
    state: Option<Res<RtgiGpu>>,
    views: Query<&NvfRtgiVolumeBindings>,
    buffers: Res<RenderAssets<GpuShaderBuffer>>,
    (cache, device, counts): (Res<PipelineCache>, Res<RenderDevice>, Res<RtgiCounts>),
    mut ctx: RenderContext,
) {
    let Some(job) = job.filter(|_| !pass.done) else { return };
    if !state.is_some_and(|s| s.held.as_ref().is_some_and(|h| h.key.solved)) {
        return;
    }
    let [volume, reads] = layouts();
    let id = *pass.pipeline.get_or_insert_with(|| {
        cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some("rtgi_readback_pipeline".into()),
            layout: vec![volume.clone(), reads.clone()],
            shader: SHADER,
            entry_point: Some("answer".into()),
            ..default()
        })
    });
    let (Some(pipeline), Some(view)) = (cache.get_compute_pipeline(id), views.iter().next()) else { return };
    let (Some(asked), Some(answers)) = (buffers.get(&job.asked), buffers.get(&job.answers)) else { return };
    let volume = device.create_bind_group(
        "rtgi_readback_volume",
        &cache.get_bind_group_layout(&volume),
        &BindGroupEntries::with_indices(((47, view.uniform.as_entire_binding()), (48, &view.texture), (49, &view.sampler))),
    );
    let reads = device.create_bind_group("rtgi_readback_reads", &cache.get_bind_group_layout(&reads), &BindGroupEntries::sequential((asked.buffer.as_entire_binding(), answers.buffer.as_entire_binding())));
    let mut compute = ctx.command_encoder().begin_compute_pass(&ComputePassDescriptor { label: Some("rtgi_readback"), timestamp_writes: None });
    compute.set_pipeline(pipeline);
    compute.set_bind_group(0, &volume, &[]);
    compute.set_bind_group(1, &reads, &[]);
    compute.dispatch_workgroups((READS as u32).div_ceil(32), 1, 1);
    counts.add(Did::Dispatch, 1);
    pass.done = true;
}
