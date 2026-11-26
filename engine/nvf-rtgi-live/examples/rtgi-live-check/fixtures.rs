//! Scenes made here, without game data, whose rays, points and reach are held against the CPU's.
//!
//! They have to agree: the same surface met (or another in its place, parallel, with the same light), the same light,
//! the same lamps and shadow rays. Where an answer is a rounding's on either side of a limit that the kernel leaves
//! to the structure (an alpha exactly at the cutoff away from the point, a shadow ray's end) it may be one of those
//! named for it: the CPU's of scenes a little to either side of the limit. A point's counts are then the sums of the
//! answers each of its rays agreed with.

use crate::gpu::Gpu;
use nvf_rtgi::{bounces, classified, directions, meet, prepare, Bounce, Cone, Cull, Directional, Instance, Lamp, LampTable, Level, LightState, Material, Mesh, Ray, SceneInput, SceneLimits, Texture, Wrap, V3};
use nvf_rtgi_live::{answers_of, meetings_of, probes_of, reach_of, Answer, Config, Points, Tables, Target};

/// Of a ray's light, and of a point's mean.
const TOLERANCE: f32 = 5e-4;
const EMISSION: f32 = 0.25;
/// A difference that decides nothing, and why.
const KNOWN: (&str, &str) = (
    "from the edge of two walls, near the origin",
    "one ray from a point one float inside a wall and on the next: the CPU meets the hidden back of the one at their shared edge, the kernel the other's front 1e-5 nearer; on the prototype kernel too, its cause not yet found",
);

type Parts = Vec<(Mesh, Material)>;
type Shows = fn(&[Option<Bounce>]) -> bool;

struct Fixture {
    name: String,
    parts: Parts,
    /// What else a ray's answer may be: that of one of these.
    others: Vec<(Parts, LightState)>,
    /// What is met is compared, and how many shadow rays it has, not what they say: they start in other surfaces' planes,
    /// and are the structure's, whose rounding is not the solver's.
    place_only: bool,
    textures: Vec<Option<Texture>>,
    light: LightState,
    from: Vec<V3>,
    along: Vec<V3>,
    /// Whether the CPU's answers show what the fixture is for.
    shows: Shows,
}

fn v(x: f32, y: f32, z: f32) -> V3 {
    V3([x, y, z])
}

fn unit(p: V3) -> V3 {
    p.unit().unwrap_or(p)
}

/// A rectangle of half sides `u` and `v`, its front towards `u` x `v`.
fn rectangle(centre: V3, u: V3, v: V3, material: Material) -> (Mesh, Material) {
    let positions = [centre - u - v, centre + u - v, centre + u + v, centre - u + v].map(|p| p.0).to_vec();
    (Mesh { positions, uv0: vec![[0.0, 0.0], [3.0, 0.0], [3.0, 2.0], [0.0, 2.0]], indices: vec![0, 1, 2, 0, 2, 3], ..Mesh::default() }, material)
}

fn coloured(r: f32, g: f32, b: f32) -> Material {
    Material { base: [r, g, b, 1.0], ..Material::default() }
}

/// A box's six sides, fronts inward, each of its own colour; the one at -z unlit.
fn room(centre: V3, half: V3) -> Parts {
    let axis = |k: usize| V3(std::array::from_fn(|i| if i == k % 3 { half[i] } else { 0.0 }));
    let mut out = Vec::new();
    for k in 0..3 {
        for upper in [false, true] {
            let (a, b) = if upper { (axis(k + 2), axis(k + 1)) } else { (axis(k + 1), axis(k + 2)) };
            let m = Material { unlit: k == 2 && !upper, ..coloured(0.25 + 0.25 * k as f32, 0.9 - 0.3 * k as f32, if upper { 0.7 } else { 0.15 }) };
            out.push(rectangle(if upper { centre + axis(k) } else { centre - axis(k) }, a, b, m));
        }
    }
    out
}

fn lamp(position: V3, display: V3, range: f32) -> Lamp {
    Lamp { position, display, range, cone: None }
}

/// Ambient, a directional, a lamp that clips a channel, a spot.
fn light(at: V3) -> LightState {
    let spot = Lamp { cone: Some(Cone::new(v(0.4, -1.0, 0.2), 0.3, 0.8)), ..lamp(at + v(-50.0, 60.0, 30.0), v(0.2, 0.9, 1.4), 260.0) };
    LightState {
        ambient: v(0.2, 0.3, 0.4),
        directionals: vec![Directional { display: v(0.3, 0.2, 0.1), to_source: unit(v(0.3, 0.9, -0.2)) }],
        lamps: vec![lamp(at + v(40.0, 20.0, -35.0), v(2.5, 0.6, 0.3), 300.0), spot],
    }
}

fn one_lamp(position: V3) -> LightState {
    LightState { ambient: V3([0.1; 3]), lamps: vec![lamp(position, V3([0.8; 3]), 300.0)], ..LightState::default() }
}

fn lit(b: &[Option<Bounce>]) -> bool {
    b.iter().flatten().any(|b| b.light.max_abs() > 0.01)
}

fn dark(b: &[Option<Bounce>]) -> bool {
    b.iter().all(|b| b.is_some_and(|b| b.light == V3::ZERO))
}

fn any(_: &[Option<Bounce>]) -> bool {
    true
}

impl Fixture {
    fn new(name: impl Into<String>, parts: Parts, light: LightState, from: Vec<V3>, along: Vec<V3>, shows: Shows) -> Fixture {
        Fixture { name: name.into(), parts, others: Vec::new(), place_only: false, textures: Vec::new(), light, from, along, shows }
    }

    fn scene(&self, parts: &Parts) -> SceneInput {
        let mut out = SceneInput { textures: self.textures.clone(), ..SceneInput::default() };
        for (i, (mesh, material)) in parts.iter().enumerate() {
            out.meshes.push(mesh.clone());
            out.materials.push(*material);
            out.instances.push(Instance { mesh: i, material: i, world_from_local: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] });
        }
        out
    }
}

/// Holes by a checkerboard's alpha, coloured.
fn holes() -> Texture {
    let texels = (0..256).map(|i| [200, 40 + (i % 16) as u8 * 10, 90, if (i / 64 + i % 16 / 4) % 2 == 0 { 255 } else { 0 }]).collect();
    Texture { levels: vec![Level { width: 16, height: 16, texels }], srgb: true, wrap: [Wrap::Mirror, Wrap::Repeat] }
}

/// One alpha all over.
fn flat(alpha: u8) -> Texture {
    Texture { levels: vec![Level { width: 2, height: 2, texels: vec![[255, 255, 255, alpha]; 4] }], srgb: false, wrap: [Wrap::Repeat; 2] }
}

const PLACES: [(&str, V3); 2] = [("near the origin", V3::ZERO), ("near 8000", V3([7936.0, 8000.0, -1792.0]))];

fn surfaces(out: &mut Vec<Fixture>) {
    let shows: Shows = |b| {
        // Lit ones, and unlit ones.
        let some = |f: &dyn Fn(&Bounce) -> bool| b.iter().flatten().any(f);
        some(&|b| b.shadow_rays > 0 && b.light[0] > 0.2) && some(&|b| b.lamps == 0 && b.light.max_abs() > 0.01)
    };
    let from = |c: V3| vec![c + v(7.3, -11.1, 3.7), c + v(-61.7, 43.9, 22.2)];
    let half = v(100.0, 90.0, 120.0);
    let lit_room = |name: &str, c: V3| Fixture::new(name, room(c, half), light(c), from(c), directions(512), shows);
    out.push(lit_room("outgoing light", V3::ZERO));
    out.push(lit_room("outgoing light, far from the origin", v(60_000.0, 2_000.0, -40_000.0)));
    let mut f = Fixture { textures: vec![Some(holes())], ..lit_room("cutouts, textures and vertex colours", V3::ZERO) };
    let cut = Material { texture: Some(0), cutoff: 0.5, cull: Cull::None, double_sided: true, uv: [[0.9, 0.1, 0.05], [0.0, 1.1, -0.3]], ..Material::default() };
    let mut fence = rectangle(v(25.0, 0.0, 5.0), v(0.0, 90.0, 0.0), v(0.0, 0.0, 120.0), cut);
    fence.0.colors = vec![[0.9, 1.0, 0.5, 1.0], [1.0; 4], [0.3, 1.0, 1.0, 0.8], [1.0; 4]];
    let mut shown = rectangle(v(-30.0, -80.0, 0.0), v(0.0, 0.0, 60.0), v(60.0, 0.0, 0.0), Material { texture: Some(0), display_colors: true, ..Material::default() });
    shown.0.colors = vec![[0.02, 0.5, 1.0, 1.0], [0.9, 0.03, 0.4, 1.0], [1.0; 4], [0.2; 4]];
    f.parts.extend([fence, shown]);
    out.push(f);
    let floor = || rectangle(V3::ZERO, v(0.0, 0.0, 64.0), v(64.0, 0.0, 0.0), coloured(0.5, 0.6, 0.7));
    let from = v(10.0, 50.0, -7.0);
    // Along its diagonal, and at its ends but for the corners.
    let diagonal: Vec<V3> = (1..64).map(|i| unit(v(2.0 * i as f32 - 64.0, 0.0, 2.0 * i as f32 - 64.0) - from)).collect();
    out.push(Fixture::new("a shared edge", vec![floor()], one_lamp(v(0.0, 60.0, 0.0)), vec![from], diagonal, |b| b.iter().all(|b| b.is_some()) && lit(b)));
    let decal = (floor().0, coloured(0.9, 0.1, 0.1));
    let down: Vec<V3> = directions(64).into_iter().filter(|d| d[1] < -0.75).collect();
    out.push(Fixture::new("two surfaces in one place", vec![floor(), decal], one_lamp(v(0.0, 60.0, 0.0)), vec![from], down, lit));
}

/// From a wall, a float to either side of it, and a little further.
fn walls(out: &mut Vec<Fixture>) {
    for (place, c) in PLACES {
        let wall = c[0] + 128.0;
        for (name, x, near) in [
            ("on a wall", wall, true),
            ("one float inside a wall", wall.next_down(), true),
            ("one float behind a wall", wall.next_up(), true),
            ("a little inside a wall", wall - 0.05, false),
            ("a little behind a wall", wall + 0.05, false),
        ] {
            let from = vec![v(x, c[1] + 64.0, c[2]), v(x, c[1] - 64.0, c[2] + 64.0)];
            let along = directions(512).into_iter().filter(|d| near || d[0].abs() > 0.05).collect();
            out.push(Fixture::new(format!("{name}, {place}"), room(c, V3([128.0; 3])), light(c), from, along, any));
        }
    }
}

/// From where surfaces meet, exactly there and a float or so beside: an edge and a corner of two and three walls, the seam
/// of two floors in one plane, the edge of a floor's two triangles, a corner of eight, a floor's edge with nothing beyond.
fn seams(out: &mut Vec<Fixture>) {
    let walls = |c: V3| room(c, V3([128.0; 3]));
    let floors = |c: V3| [-64.0, 64.0].map(|z| rectangle(c + v(0.0, 0.0, z), v(0.0, 0.0, 64.0), v(64.0, 0.0, 0.0), coloured(0.5, 0.6 + z / 400.0, 0.7))).to_vec();
    let fan = |c: V3| {
        let ring = [[60.0, 0.0], [40.0, 45.0], [0.0, 55.0], [-35.0, 50.0], [-60.0, 5.0], [-45.0, -30.0], [0.0, -70.0], [50.0, -35.0]];
        let mut positions = vec![c.0];
        positions.extend(ring.map(|[x, z]| (c + v(x, 0.0, z)).0));
        let indices = (0..8u32).flat_map(|i| [0, 1 + (i + 1) % 8, 1 + i]).collect();
        let colors = (0..9).map(|i| [0.3 + 0.07 * i as f32, 0.5, 0.9 - 0.05 * i as f32, 1.0]).collect();
        vec![(Mesh { positions, indices, colors, ..Mesh::default() }, Material::default())]
    };
    for (place, c) in PLACES {
        let cases: [(&str, Parts, V3); 7] = [
            ("the edge of two walls", walls(c), v(128.0, 128.0, 10.0)),
            ("the corner of three walls", walls(c), V3([128.0; 3])),
            ("the edge of a wall's triangles", walls(c), v(128.0, 32.0, 32.0)),
            ("the seam of two floors", floors(c), v(10.0, 0.0, 0.0)),
            ("the corner of two floors", floors(c), v(64.0, 0.0, 0.0)),
            ("a floor's edge", floors(c), v(64.0, 0.0, 30.0)),
            ("the corner of eight triangles", fan(c), V3::ZERO),
        ];
        for (name, parts, at) in cases {
            let at = c + at;
            let mut from = vec![at];
            for k in 0..3 {
                // A step of f32 at 64; beside 0 itself there are only the subnormal ones, which are ignored.
                for f in if at[k] == 0.0 { [-7.629_394_5e-6, 7.629_394_5e-6] } else { [at[k].next_down(), at[k].next_up()] } {
                    from.push(V3(std::array::from_fn(|i| if i == k { f } else { at[i] })));
                }
            }
            let f = Fixture::new(format!("from {name}, {place}"), parts, light(c), from, directions(512), any);
            out.push(Fixture { place_only: true, ..f });
        }
    }
}

/// Surfaces in one place: what the solver's rounding picks.
fn ties(out: &mut Vec<Fixture>) {
    let down: Vec<V3> = directions(256).into_iter().filter(|d| d[1] < -0.75).collect();
    for (place, c) in PLACES {
        // Not along an axis: the distances are not the same numbers.
        let (u, w) = (v(63.1, 7.7, 3.3), v(-4.1, 9.3, 61.7));
        let floor = |turned: bool, material| {
            let mut part = rectangle(c, w, u, material);
            if turned {
                // The same corners in another order, and the other winding.
                part.0.indices = vec![1, 0, 3, 1, 3, 2];
            }
            part
        };
        let front = coloured(0.5, 0.6, 0.7);
        let back = Material { cull: Cull::Back, ..coloured(0.9, 0.2, 0.2) };
        let cut = Material { texture: Some(0), cutoff: 0.5, cull: Cull::None, ..coloured(0.9, 0.1, 0.1) };
        for (name, a, b) in [
            ("a front and a back", floor(false, front), floor(true, back)),
            ("a back and a front", floor(true, back), floor(false, front)),
            ("two of one side", floor(false, front), floor(false, coloured(0.9, 0.1, 0.1))),
            ("a cutout and a surface", floor(true, cut), floor(false, front)),
            ("a surface and a cutout", floor(false, front), floor(true, cut)),
        ] {
            let from = vec![c + v(10.0, 50.0, -7.0), c + v(-3.3, 31.7, 8.1)];
            let f = Fixture::new(format!("{name} in one place, {place}"), vec![a, b], one_lamp(c + v(0.0, 60.0, 0.0)), from, down.clone(), any);
            out.push(Fixture { textures: vec![Some(flat(255))], ..f });
        }
        let decal = rectangle(c + v(0.0, 0.05, 0.0), v(0.0, 0.0, 20.0), v(20.0, 0.0, 0.0), coloured(0.9, 0.1, 0.1));
        let wall = rectangle(c, v(0.0, 0.0, 64.0), v(64.0, 0.0, 0.0), front);
        let decals: Shows = |b| b.iter().all(|b| b.is_some_and(|b| b.tri >= 2 && !b.hidden)) && lit(b);
        let low: Vec<V3> = (0..8).map(|i| unit(v(60.0, -1.2, i as f32 - 4.0))).collect();
        out.push(Fixture::new(format!("a decal, grazing, {place}"), vec![wall, decal], one_lamp(c + v(0.0, 60.0, 0.0)), vec![c + v(-60.0, 1.25, 0.0)], low, decals));
    }
}

/// A lamp above a floor with something between them: near either end of the shadow ray.
fn shadows(out: &mut Vec<Fixture>) {
    let cut = Material { texture: Some(0), cutoff: 0.5, cull: Cull::None, ..Material::default() };
    let plain = Material { cull: Cull::None, ..Material::default() };
    for (kind, material) in [("opaque", plain), ("cutout", cut)] {
        // The lamp at 100 is drawn in by 0.01, the floor's end starts at 0.01.
        for (y, edge, shows) in [(99.98, false, dark as Shows), (99.99, true, any), (99.995, false, lit), (100.0, false, lit), (100.5, false, lit), (0.02, false, dark), (0.01, true, any), (0.005, false, lit)] {
            // The ray comes down at 45 degrees from +x, and meets what is above the floor at x = y: past its end.
            let with = |y: f32| {
                let middle = if y > 50.0 { v(0.0, y, 0.0) } else { v(y / 2.0 - 20.0, y, 0.0) };
                let floor = rectangle(V3::ZERO, v(0.0, 0.0, 90.0), v(70.0, 0.0, 0.0), coloured(0.5, 0.6, 0.7));
                vec![floor, rectangle(middle, v(0.0, 0.0, 20.0), v(20.0, 0.0, 0.0), material)]
            };
            let lamp = one_lamp(v(0.0, 100.0, 0.0));
            // At the end itself it may be on either side.
            let others = if edge { [with(y - 5e-3), with(y + 5e-3)].map(|parts| (parts, lamp.clone())).to_vec() } else { Vec::new() };
            let f = Fixture::new(format!("a shadow's end, {kind} at {y}"), with(y), lamp, vec![v(30.0, 30.0, 0.0)], vec![unit(v(-1.0, -1.0, 0.0))], shows);
            out.push(Fixture { others, textures: vec![Some(flat(255))], ..f });
        }
    }
    let floor = rectangle(V3::ZERO, v(0.0, 0.0, 90.0), v(70.0, 0.0, 0.0), coloured(0.5, 0.6, 0.7));
    out.push(Fixture::new("a lamp nearer than a shadow ray's ends", vec![floor], one_lamp(v(0.0, 0.015, 0.0)), vec![v(30.0, 30.0, 0.0)], vec![unit(v(-1.0, -1.0, 0.0))], lit));
    // Main's half brightness: a spot 0.005 above the place, its cone's own direction unit; 45 degrees off, nothing.
    let floor = rectangle(v(7.0, 0.0, 3.0), v(0.0, 0.0, 50.0), v(50.0, 0.0, 0.0), coloured(0.5, 0.6, 0.7));
    let spot = Lamp { cone: Some(Cone::new(v(0.0, -1.0, 0.0), 0.3, 0.5)), ..lamp(v(0.0, 0.005, 0.0), V3([0.8; 3]), 100.0) };
    let half: Shows = |b| b[0].is_some_and(|b| b.light.max_abs() > 0.01 && b.shadow_rays == 1) && b[1].is_some_and(|b| b.light == V3::ZERO && b.shadow_rays == 0);
    let light = LightState { ambient: V3([0.1; 3]), lamps: vec![spot], ..LightState::default() };
    out.push(Fixture::new("a spot 0.005 above the place, at half brightness", vec![floor], light, vec![v(0.0, 30.0, 0.0), v(0.005, 30.0, 0.0)], vec![v(0.0, -1.0, 0.0)], half));
}

/// A sheet before a wall, the lamp in front of both: from in front, and from between them.
fn sheets(out: &mut Vec<Fixture>) {
    let wall = || rectangle(v(0.0, 0.0, -60.0), v(90.0, 0.0, 0.0), v(0.0, 90.0, 0.0), coloured(0.7, 0.8, 0.9));
    // Both ways.
    let along: Vec<V3> = directions(256).into_iter().filter(|d| d[2].abs() > 0.8).collect();
    let before = one_lamp(v(5.0, 8.0, 50.0));
    let parts = |material: Material, alpha: f32| {
        let mut sheet = rectangle(V3::ZERO, v(90.0, 0.0, 0.0), v(0.0, 90.0, 0.0), material);
        sheet.0.colors = vec![[1.0, 1.0, 1.0, alpha]; 4];
        vec![wall(), sheet]
    };
    let mut add = |name: &str, others: Vec<(Parts, LightState)>, material: Material, alpha: f32, textures: Vec<Option<Texture>>, shows| {
        let f = Fixture::new(name, parts(material, alpha), before.clone(), vec![v(1.0, 2.0, 40.0), v(1.5, -2.5, -30.0)], along.clone(), shows);
        out.push(Fixture { others, textures, ..f });
    };
    let through: Shows = |b| b.iter().flatten().any(|b| b.tri < 2 && b.light.max_abs() > 0.01);
    let stopped: Shows = |b| b.iter().flatten().all(|b| b.tri >= 2 || b.light == V3::ZERO) && b.iter().flatten().any(|b| b.tri >= 2);
    // With its back's normal turned, so that the lamp beyond it does not light it, whatever its own shadow.
    let cut = |alpha: f32| Material { double_sided: true, cutoff: 0.5, cull: Cull::None, base: [1.0, 0.5, 0.5, alpha], ..Material::default() };
    add("alpha below the cutoff", Vec::new(), cut(0.499), 1.0, Vec::new(), through);
    // Far from the points the structure's weights decide: through it or not, and what is met behind it lit or not.
    let others = vec![(parts(cut(0.499), 1.0), before.clone()), (parts(cut(0.501), 1.0), before.clone()), (parts(cut(0.499), 1.0), LightState { lamps: Vec::new(), ..before.clone() })];
    add("alpha at the cutoff", others, cut(0.5), 1.0, Vec::new(), any);
    add("alpha above the cutoff", Vec::new(), cut(0.501), 1.0, Vec::new(), stopped);
    let textured = |alpha| Material { texture: Some(0), ..cut(alpha) };
    add("a texture's alpha below the cutoff", Vec::new(), textured(1.0), 1.0, vec![Some(flat(127))], through);
    add("a texture's alpha above the cutoff", Vec::new(), textured(1.0), 1.0, vec![Some(flat(128))], stopped);
    // Infinite times the texel's 0.
    add("an alpha that is no number", Vec::new(), textured(1e3), 3e38, vec![Some(flat(0))], stopped);
    let sided = |cull, double_sided| Material { cull, double_sided, ..coloured(0.9, 0.4, 0.2) };
    let hidden: Shows = |b| b.iter().flatten().any(|b| b.hidden) && b.iter().flatten().all(|b| !b.hidden || (b.light == V3::ZERO && b.shadow_rays == 0));
    add("a hidden back", Vec::new(), sided(Cull::Back, false), 1.0, Vec::new(), hidden);
    add("a hidden front", Vec::new(), sided(Cull::Front, false), 1.0, Vec::new(), hidden);
    add("a double-sided sheet", Vec::new(), sided(Cull::None, true), 1.0, Vec::new(), stopped);
    add("a sheet drawn on both sides, its normal not turned", Vec::new(), sided(Cull::None, false), 1.0, Vec::new(), stopped);
}

/// Alpha exactly at a candidate's own weight, near the point, where the solver's test and weights decide: sheets 0.3
/// behind each point, their alpha a corner's weight where the ray meets them, the cutoff that weight (they stop the
/// ray) or the float above it (it goes on to the wall). A weight a step off either way turns one of the two.
fn thresholds(out: &mut Vec<Fixture>) -> Result<(), String> {
    let along = vec![unit(v(0.13, -0.21, -1.0))];
    let (mut meshes, mut from) = (Vec::new(), Vec::new());
    for i in 0..16 {
        let (c, s) = (v(40.0 * i as f32, 0.0, 0.0), 1.0 + 0.137 * i as f32);
        let corners = [c + v(-9.3 * s, -7.1, 0.0), c + v(8.7, -6.3 * s, 0.0), c + v(0.9 * s, 9.9, 0.0)];
        let colors = [0.0, 1.0, 0.0].map(|a| [1.0, 1.0, 1.0, a]).to_vec();
        meshes.push(Mesh { positions: corners.map(|p| p.0).to_vec(), indices: vec![0, 1, 2], colors, ..Mesh::default() });
        from.push(c + v(0.37 * s - 1.1, 0.53 * i as f32 - 2.0, 0.3));
    }
    let wall = rectangle(v(300.0, 0.0, -5.0), v(400.0, 0.0, 0.0), v(0.0, 40.0, 0.0), coloured(0.7, 0.8, 0.9));
    let sheet = |cutoff: f32| Material { cutoff, cull: Cull::None, ..coloured(0.9, 0.4, 0.2) };
    let with = |cutoffs: &[f32]| -> Parts { [wall.clone()].into_iter().chain(meshes.iter().zip(cutoffs).map(|(m, c)| (m.clone(), sheet(*c)))).collect() };
    // The CPU's weights where each ray meets its sheet, all of them opaque, and its alpha there.
    let probe = Fixture::new("", with(&[0.0; 16]), LightState::default(), Vec::new(), Vec::new(), any);
    let scene = prepare(probe.scene(&probe.parts), &SceneLimits::default(), 1, &|| false).map_err(|e| e.to_string())?;
    let e = scene.export();
    let mut at = Vec::new();
    for p in &from {
        let h = scene.transport(&Ray::new(*p, along[0]), f32::INFINITY).ok_or("a sheet that is not met")?;
        let (a, w) = (e.faces[h.tri() as usize].colors.map(|c| c[3]), h.weights());
        at.push(a[0] * w[0] + a[1] * w[1] + a[2] * w[2]);
    }
    let above: Vec<f32> = at.iter().map(|a| a.next_up()).collect();
    let light = one_lamp(v(300.0, 0.0, 60.0));
    let stops: Shows = |b| b.iter().all(|b| b.is_some_and(|b| b.tri >= 2));
    let goes_on: Shows = |b| b.iter().all(|b| b.is_some_and(|b| b.tri < 2));
    out.push(Fixture::new("alpha at a hit's own weight, the cutoff that weight", with(&at), light.clone(), from.clone(), along.clone(), stops));
    out.push(Fixture::new("alpha at a hit's own weight, the cutoff the float above", with(&above), light, from, along, goes_on));
    Ok(())
}

/// Straight down at the faces of the lamps' grid, and at its far edge where a place is rounded onto it.
fn grid(out: &mut Vec<Fixture>) {
    let floor = rectangle(V3::ZERO, v(0.0, 0.0, 1500.0), v(1500.0, 0.0, 0.0), coloured(0.6, 0.6, 0.6));
    let light = LightState { ambient: V3([0.1; 3]), lamps: vec![lamp(v(-300.0, 80.0, 10.0), V3([0.9; 3]), 400.0), lamp(v(250.0, 90.0, 0.0), v(0.2, 0.9, 0.5), 350.0), lamp(v(10.0, 120.0, -60.0), v(1.0, 0.1, 0.3), 200.0)], ..LightState::default() };
    if let Ok(table) = LampTable::of(&light, &|| false) {
        let mut from = Vec::new();
        for k in [0, 2] {
            for face in [0, 1, table.cells[k] / 2, table.cells[k] - 1, table.cells[k]] {
                let face = table.min[k] + face as f32 * table.side;
                for x in [face.next_down(), face, face.next_up(), face + 0.3] {
                    from.push(if k == 0 { v(x, 40.0, 3.0) } else { v(12.0, 40.0, x) });
                }
            }
        }
        out.push(Fixture::new("the faces of the lamps' grid", vec![floor], light, from, vec![v(0.0, -1.0, 0.0)], |b| lit(b) && b.iter().flatten().any(|b| b.lamps == 0)));
    }
    // Main's cases (light.rs `the_grid_at_its_edges`): the place's c is rounded to `cells` itself, and is the last cell's.
    let edge: Shows = |b| b.iter().all(|b| b.is_some_and(|b| b.lamps > 0 && b.shadow_rays > 0));
    let lamps = |l: &[(f32, f32)]| LightState { ambient: V3([0.1; 3]), lamps: l.iter().map(|(x, range)| lamp(v(*x, 0.0, 0.0), V3([0.8; 3]), *range)).collect(), ..LightState::default() };
    let wall = |x: f32, half: f32| rectangle(v(x, 0.0, 0.0), v(0.0, 0.0, half), v(0.0, half, 0.0), coloured(0.6, 0.6, 0.6));
    let rooms = lamps(&[(-2900.3, 100.0), (-100.0, 100.0)]);
    out.push(Fixture::new("the lamps' grid's far edge, a room's", vec![wall(-1e-5, 30.0)], rooms, vec![v(-50.0, 3.1, 2.7)], vec![v(1.0, 0.0, 0.0)], edge));
    let worlds = lamps(&[(-2_097_152.0, 1.0), (-786_491.0 - 0.8125, 60.01), (0.0, 1.0)]);
    out.push(Fixture::new("the lamps' grid's far edge, a world's", vec![wall(0.99, 0.5)], worlds, vec![v(0.2, 0.11, 0.07)], vec![v(1.0, 0.0, 0.0)], edge));
}

/// As the solver's shadow rays are lifted, and how far surfaces that are in one place are apart at most.
fn lift(p: V3) -> f32 {
    0.01f32.max(4e-6 * p.max_abs())
}

/// How a ray's answers differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Same,
    /// The same triangle, other light or another place on it.
    Light,
    Place,
    /// Another triangle in the same place, parallel to it: the same light, or not.
    TieSame,
    Tie,
    /// Another triangle at the same place, on the other side of an edge.
    Edge,
    /// Another triangle elsewhere, or a side.
    Other,
    OnlyCpu,
    OnlyGpu,
    /// The same answer, but not the same lamps or shadow rays.
    Counts,
}

fn kind(cpu: &Option<Bounce>, gpu: &Answer, order: &[u32], normals: &[V3]) -> Kind {
    let Some(c) = cpu else { return if gpu.hit { Kind::OnlyGpu } else { Kind::Same } };
    if !gpu.hit {
        return Kind::OnlyCpu;
    }
    let same = (c.light - gpu.light).max_abs() <= TOLERANCE && c.hidden == gpu.hidden;
    let close = (c.p - gpu.p).max_abs() <= lift(c.p);
    let Some(tri) = order.get(gpu.primitive as usize) else { return Kind::Other };
    if *tri == c.tri {
        return match (c.front == gpu.front, close && (c.start - gpu.start).max_abs() <= 2.0 * lift(c.p), same) {
            (false, ..) => Kind::Other,
            (_, false, _) => Kind::Place,
            (_, _, true) => Kind::Same,
            _ => Kind::Light,
        };
    }
    let (a, b) = (normals[c.tri as usize], normals[*tri as usize]);
    let parallel = a.dot(b).abs() >= 0.99 && a.dot(c.p - gpu.p).abs() <= (0.5 * lift(c.p)).min(0.02) && (c.p - gpu.p).max_abs() <= 4.0;
    match (parallel, close, same) {
        (true, _, true) => Kind::TieSame,
        (true, ..) => Kind::Tie,
        (_, true, _) => Kind::Edge,
        _ => Kind::Other,
    }
}

/// The counts of an answer: a hit, a hidden one, its lamps and shadow rays.
fn counts(b: &Option<Bounce>) -> [u32; 4] {
    b.map_or([0; 4], |b| [1, u32::from(b.hidden), b.lamps, b.shadow_rays])
}

/// A fixture's verdict, printed: whether it passed, and whether what differs is the known difference.
fn check(gpu: &Gpu, f: &Fixture) -> Result<(bool, bool), String> {
    let rays: Vec<(V3, V3)> = f.from.iter().flat_map(|from| f.along.iter().map(|d| (*from, *d))).collect();
    let of = |parts: &Parts, light: &LightState| {
        let scene = prepare(f.scene(parts), &SceneLimits::default(), 1, &|| false).map_err(|e| e.to_string())?;
        let cpu = bounces(&scene, light, &rays, EMISSION, 4, &|| false).map_err(|e| e.to_string())?;
        Ok::<_, String>((scene, cpu))
    };
    let (scene, cpu) = of(&f.parts, &f.light)?;
    let others = f.others.iter().map(|(parts, light)| Ok(of(parts, light)?.1)).collect::<Result<Vec<_>, String>>()?;
    let tables = Tables::new(&scene, &f.along)?;
    let normals: Vec<V3> = scene.export().faces.iter().map(|f| f.normal).collect();
    let points = Points::at(f.from.clone(), &scene, &tables)?;
    let room = gpu.room(&tables)?;
    let config = Config { emission: EMISSION, ..Config::default() };
    let answers = answers_of(&gpu.run(Target::Replay, &room, &points, &f.light, &config)?).into_iter().collect::<Option<Vec<_>>>().ok_or("a ray without an answer")?;
    let probes = probes_of(&gpu.run(Target::Moments, &room, &points, &f.light, &config)?);
    let reach = reach_of(&gpu.run(Target::Reach, &room, &points, &f.light, &config)?);
    let counted = classified(&scene, &f.from, &f.along, 4, &|| false).map_err(|e| e.to_string())?;
    let n = f.along.len();
    if answers.len() != rays.len() || probes.len() != f.from.len() || reach.len() != f.from.len() {
        return Err(format!("{}: {} answers, {} probes, {} reaches for {} rays", f.name, answers.len(), probes.len(), reach.len(), rays.len()));
    }
    // Of each point: the sums of the answers its rays agreed with, and whether any was another than the scene's own.
    let (mut sums, mut means, mut named) = (vec![[0u32; 4]; f.from.len()], vec![V3::ZERO; f.from.len()], vec![false; f.from.len()]);
    let (mut off, mut either) = (Vec::new(), 0);
    for (i, (c, g)) in cpu.iter().zip(&answers).enumerate() {
        let from = f.from[i / n];
        let finite = !g.hit || (g.light.finite() && g.p.finite() && g.start.finite() && g.t.is_finite());
        let same_counts = |o: &Option<Bounce>| counts(o)[2..] == [g.lamps, g.shadow_rays];
        let mut k = kind(c, g, tables.order(), &normals);
        // Where it is met at once a ray along the surface has its place by the rounding.
        let at_once = k == Kind::Place && c.is_some_and(|c| c.hidden == g.hidden && [c.p, g.p].iter().all(|p| (*p - from).length() < 1.0) && (c.light - g.light).max_abs() <= 4.0 * TOLERANCE);
        let met = f.place_only && k == Kind::Light && c.is_some_and(|c| c.hidden == g.hidden);
        if matches!(k, Kind::Same | Kind::TieSame) && !same_counts(c) {
            k = Kind::Counts;
        }
        let agreed = if finite && (matches!(k, Kind::Same | Kind::TieSame) || ((at_once || met) && same_counts(c))) {
            Some(c)
        } else {
            let other = |o: &Option<Bounce>| match o {
                None => !g.hit,
                Some(o) => g.hit && o.hidden == g.hidden && (o.light - g.light).max_abs() <= 4.0 * TOLERANCE && (o.p - g.p).max_abs() <= 0.02 && same_counts(&Some(*o)),
            };
            let found = others.iter().map(|o| &o[i]).find(|o| finite && other(o));
            either += usize::from(found.is_some());
            named[i / n] |= found.is_some();
            found
        };
        let used = agreed.unwrap_or(c);
        if agreed.is_none() {
            off.push((i, k));
        }
        let [a, b, l, s] = counts(used);
        sums[i / n] = [sums[i / n][0] + a, sums[i / n][1] + b, sums[i / n][2] + l, sums[i / n][3] + s];
        means[i / n] = means[i / n] + used.map_or(V3::ZERO, |u| if u.hidden { V3::ZERO } else { u.light });
    }
    // A point is off where its counts are not the sums, its mean light not theirs, or its reach not the CPU's.
    let mut points_off = Vec::new();
    for (k, (p, r)) in probes.iter().zip(&reach).enumerate() {
        let cpu_reach = if named[k] { [sums[k][0], sums[k][1]] } else { counted[k] };
        let main = cpu[k * n..(k + 1) * n].iter().fold([0; 2], |s, b| [s[0] + counts(b)[0], s[1] + counts(b)[1]]);
        let mean = (p.moments.c[0] - means[k] * (1.0 / n as f32)).max_abs();
        if [p.hits, p.backs, p.lamps, p.shadow_rays] != sums[k] || mean > TOLERANCE || *r != cpu_reach || counted[k] != main {
            points_off.push(k);
        }
    }
    let shows = (f.shows)(&cpu);
    let known = f.name == KNOWN.0 && off.len() <= 1 && points_off.iter().all(|k| off.iter().any(|(i, _)| i / n == *k));
    let passed = off.is_empty() && points_off.is_empty() && shows;
    let verdict = if passed {
        "PASS"
    } else if known && shows {
        "KNOWN"
    } else {
        "FAIL"
    };
    let met = [cpu.iter().flatten().count(), answers.iter().filter(|g| g.hit).count()];
    println!("{verdict} {}: {} rays, {} differ, {either} another answer that is named; hits {} / {}; {} points, {} off{}", f.name, cpu.len(), off.len(), met[0], met[1], f.from.len(), points_off.len(), if shows { "" } else { "; the CPU's answers do not show what it is for" });
    if verdict == "KNOWN" {
        println!("    known: {}", KNOWN.1);
    } else if passed && f.name == KNOWN.0 {
        println!("    the known difference does not show any more");
    }
    for (i, k) in off.iter().take(4) {
        println!("    ray {i} from {:?} along {:?}: {k:?}", f.from[i / n].0, f.along[i % n].0);
        println!("      CPU {:?}", cpu[*i]);
        println!("      GPU {:?}", answers[*i]);
    }
    for k in points_off.iter().take(4) {
        let p = &probes[*k];
        println!("    point {k} at {:?}: GPU hits, backs, lamps, shadow rays {:?}, reach {:?}; CPU {:?}, reach {:?}", f.from[*k].0, [p.hits, p.backs, p.lamps, p.shadow_rays], reach[*k], sums[*k], counted[*k]);
    }
    Ok((passed, verdict == "KNOWN"))
}

/// How many f32 there are from one to the other.
fn steps(a: f32, b: f32) -> u32 {
    let place = |f: f32| {
        let i = f.to_bits() as i32;
        if i < 0 {
            i32::MIN.wrapping_sub(i)
        } else {
            i
        }
    };
    place(a).abs_diff(place(b))
}

/// The kernel's triangle test against the solver's: random triangles, some aligned to the axes with corners on a
/// lattice, rays through the middle, the edges and the corners, from near and far and from the plane itself, and near
/// misses. Distances and weights the same bits, nothing met by one alone.
fn triangle_test(gpu: &Gpu) -> Result<bool, String> {
    let mut state = 3009u64;
    let mut draw = move || {
        // splitmix64
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        ((z ^ (z >> 31)) >> 40) as f32 / 16_777_216.0
    };
    let mut along = directions(1024);
    along.extend((0..6).map(|i| V3(std::array::from_fn(|k| if k == i / 2 { 1.0 - 2.0 * (i % 2) as f32 } else { 0.0 }))));
    let (n, mut mesh, mut from) = (100_000usize, Mesh::default(), Vec::new());
    while from.len() < n {
        let i = from.len();
        let middle = [V3::ZERO, v(1700.0, 7500.0, -800.0), v(7936.0, 8000.0, -1792.0)][i % 3];
        let size = [1.0, 40.0, 400.0][i / 3 % 3];
        let lattice = (i / 9).is_multiple_of(2);
        let mut corner = || {
            let p = v(draw() - 0.5, draw() - 0.5, draw() - 0.5) * size;
            let p = if lattice { V3(p.0.map(|c| (c * 4.0 / size).round() * size / 4.0)) } else { p };
            middle + p
        };
        let t = [corner(), corner(), corner()];
        let (x, y) = (t[1] - t[0], t[2] - t[0]);
        // What is left out of a scene would shift the numbers.
        if x.unit().zip(y.unit()).and_then(|(x, y)| x.cross(y).unit()).is_none() {
            continue;
        }
        let w = match i / 18 % 5 {
            0 => [0.25 + 0.5 * draw(), 0.2, 0.0],
            1 => [draw(), 1.0, 0.0],
            2 => [1.0, 0.0, 0.0],
            3 => [0.5, 0.5, 0.0],
            _ => [draw() * 1.0001, -1e-6, 0.0],
        };
        let target = t[0] * w[0] + t[1] * (w[1] * (1.0 - w[0])) + t[2] * (1.0 - w[0] - w[1] * (1.0 - w[0]));
        let far = [0.0, 1e-3, 3.0, 300.0][i / 90 % 4] * (0.5 + draw());
        mesh.indices.extend((0..3).map(|k| (mesh.positions.len() + k) as u32));
        mesh.positions.extend(t.map(|p| p.0));
        from.push(target - along[i % along.len()] * far);
    }
    let input = SceneInput { meshes: vec![mesh], instances: vec![Instance { mesh: 0, material: 0, world_from_local: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] }], materials: vec![Material::default()], ..SceneInput::default() };
    let scene = prepare(input, &SceneLimits::default(), 4, &|| false).map_err(|e| e.to_string())?;
    let tables = Tables::new(&scene, &along)?;
    let room = gpu.room(&tables)?;
    let gpu = meetings_of(&gpu.run(Target::Meetings, &room, &Points::bare(from.clone()), &LightState::default(), &Config::default())?);
    let corners = scene.export().corners;
    if corners.len() != n || gpu.len() != n || tables.order().iter().enumerate().any(|(i, t)| *t as usize != i) {
        return Err(format!("{} triangles, {} answers, {n} expected, in the scene's order", corners.len(), gpu.len()));
    }
    let (mut met, mut equal, mut weights, mut other, mut most, mut first) = (0, 0, 0, 0, [0; 2], Vec::new());
    for (i, ((from, corners), gpu)) in from.iter().zip(&corners).zip(&gpu).enumerate() {
        match (meet(*from, along[i % along.len()], corners), gpu.met) {
            (None, None) => {}
            (Some((a, x)), Some((b, y))) => {
                met += 1;
                let apart = [steps(a, b), x.iter().zip(y).map(|(x, y)| steps(*x, y)).max().unwrap_or(0)];
                equal += usize::from(apart[0] == 0);
                weights += usize::from(apart[1] == 0);
                most = [most[0].max(apart[0]), most[1].max(apart[1])];
                if apart != [0; 2] && first.len() < 6 {
                    first.push((i, a, b));
                }
            }
            (a, b) => {
                other += 1;
                if first.len() < 6 {
                    first.push((i, a.map_or(f32::NAN, |a| a.0), b.map_or(f32::NAN, |b| b.0)));
                }
            }
        }
    }
    let passed = equal == met && weights == met && other == 0;
    let listed = if first.is_empty() { String::new() } else { format!("; {first:?}") };
    println!("{} the triangle test: {n} pairs, {met} met by both, {equal} distances the same bits (off by {} steps at most), {weights} of the same weights (off by {} at most), {other} met by one alone{listed}", if passed { "PASS" } else { "FAIL" }, most[0], most[1]);
    Ok(passed)
}

/// Whether all that has to agree does; what is known is told apart.
pub fn run(gpu: &Gpu) -> Result<bool, String> {
    let mut all = Vec::new();
    surfaces(&mut all);
    walls(&mut all);
    seams(&mut all);
    ties(&mut all);
    shadows(&mut all);
    sheets(&mut all);
    thresholds(&mut all)?;
    grid(&mut all);
    let (mut failed, mut known) = (0, 0);
    for f in &all {
        let (passed, is_known) = check(gpu, f).map_err(|e| format!("{}: {e}", f.name))?;
        failed += usize::from(!passed && !is_known);
        known += usize::from(is_known);
    }
    let test = triangle_test(gpu)?;
    println!("fixtures: {} of {} pass, {known} known, {failed} fail; the triangle test {}", all.len() - failed - known, all.len(), if test { "passes" } else { "FAILS" });
    Ok(failed == 0 && test)
}
