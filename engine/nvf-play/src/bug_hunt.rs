//! The bug hunter: a scan of the loaded world for collision and clipping problems, for
//! the owner's own testing [design: owner 2026-11-21, "a bug hunter tool ... to check for
//! collision issues and texture clipping"]. Local only, like the Bug Log (`crate::bug_log`):
//! no network code; it writes a report under `<home>\bug-hunt\`.
//!
//! Console:
//! - `bughunt` — every reference in the loaded cells; `bughunt <radius>` — within that
//!   many game units of the player.
//! - `bughunt notes [radius]` — round each open Bug Log note in the current worldspace or
//!   interior (default radius [`NOTE_RADIUS`]); notes elsewhere are listed as not loaded.
//! - `bughunt show` / `bughunt off` — the last findings drawn as boxes in the world (on
//!   after every hunt), coloured by kind.
//!
//! What it compares, for every placed reference (REFR) that physics loaded and that is
//! drawn: the box round its drawn shapes (their render bounds) with the box round its
//! collision (`fnv_phys::PhysWorld::ref_boxes`), and both with what is under them (rays).
//! The kinds ([`Kind`]):
//! - **NoCollision**: a solid kind of object (STAT, SCOL, FURN, CONT, DOOR, TERM, ACTI)
//!   with a sizeable model and no collision at all. A suspect, not proof: some models
//!   ship without collision [hypothesis: most of the original's solid statics have it].
//! - **Misaligned**: collision and model boxes do not overlap, or their centres are apart
//!   by more than half the model's size.
//! - **CollisionTooBig**: collision reaches far past the model (2x and 64 units on an
//!   axis), the shape of the Fallout 3 x7 strips bug (LOCAL-CHANGES 12).
//! - **Floating**: a loose item, container or furniture with nothing under it within
//!   [`ITEM_GAP`] units; outdoors also a static whose model's bottom is over [`STATIC_GAP`]
//!   above the terrain at all five sample points and with nothing else under it.
//! - **Sunk**: outdoors, a static whose top is over [`SUNK_DEPTH`] under the terrain at
//!   every sample point (buried: never seen). Plants ([`is_plant`]) are not checked for
//!   missing collision.
//! - **FellThrough**: a dynamic body whose collision is wholly under the terrain.
//! - **Interpenetrating**: a dynamic body overlapping the world by over [`PENETRATION`]
//!   units in the last physics step (`ref_contacts`).
//! - **NotInPhysics**: a solid object drawn within [`PHYS_REACH`] of the player that
//!   physics does not list among its loaded cells' references, so it has no collision at
//!   all whatever its model carries.
//!
//! Cost: nothing until asked; a hunt is one frame of rays (a few per reference).

use crate::phys::PhysState;
use crate::space;
use crate::{Location, PlayWorld, RefVisual};
use bevy::camera::primitives::Aabb;
use bevy::prelude::*;
use fnv_phys::{Filter, Key, Owner};
use serde::Serialize;
use std::collections::HashMap;

/// Radius round a Bug Log note, game units [design: a room or a yard].
pub const NOTE_RADIUS: f32 = 768.0;
/// A loose item further than this over what is under it floats, game units [design].
pub const ITEM_GAP: f32 = 6.0;
/// A static further than this over the terrain (and nothing else) floats [design: well
/// past the terrain's own shell and LAND's 8-unit steps].
pub const STATIC_GAP: f32 = 24.0;
/// A model whose top is this far under the terrain is buried [design: past LAND's steps].
pub const SUNK_DEPTH: f32 = 8.0;
/// A drawn solid this near the player (horizontally) that physics does not list is
/// reported as NotInPhysics, game units [design: well inside the cells physics loads round
/// the player].
pub const PHYS_REACH: f32 = 2048.0;
/// Overlap deeper than this is interpenetration, game units [design: past contact shells].
pub const PENETRATION: f32 = 3.0;
/// A model smaller than this on its longest side is not checked for missing collision
/// (small clutter often has none) [design].
pub const SOLID_MIN_SIZE: f32 = 48.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, PartialOrd, Ord)]
pub enum Kind {
    NoCollision,
    Misaligned,
    CollisionTooBig,
    Floating,
    Sunk,
    FellThrough,
    Interpenetrating,
    NotInPhysics,
}

impl Kind {
    pub const ALL: [Kind; 8] = [Kind::NoCollision, Kind::Misaligned, Kind::CollisionTooBig, Kind::Floating, Kind::Sunk, Kind::FellThrough, Kind::Interpenetrating, Kind::NotInPhysics];

    fn colour(self) -> Color {
        match self {
            Kind::NoCollision => Color::srgb(1.0, 0.2, 1.0),
            Kind::Misaligned => Color::srgb(1.0, 0.6, 0.1),
            Kind::CollisionTooBig => Color::srgb(1.0, 0.1, 0.1),
            Kind::Floating => Color::srgb(0.2, 0.8, 1.0),
            Kind::Sunk => Color::srgb(0.6, 0.4, 0.2),
            Kind::FellThrough => Color::srgb(1.0, 1.0, 0.2),
            Kind::Interpenetrating => Color::srgb(0.3, 1.0, 0.3),
            Kind::NotInPhysics => Color::srgb(1.0, 1.0, 1.0),
        }
    }
}

/// A box in game space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bounds {
    const EMPTY: Bounds = Bounds { min: [f32::MAX; 3], max: [f32::MIN; 3] };

    fn add(&mut self, p: [f32; 3]) {
        self.min = std::array::from_fn(|i| self.min[i].min(p[i]));
        self.max = std::array::from_fn(|i| self.max[i].max(p[i]));
    }

    fn is_empty(&self) -> bool {
        self.min[0] > self.max[0]
    }

    pub fn center(&self) -> [f32; 3] {
        std::array::from_fn(|i| (self.min[i] + self.max[i]) * 0.5)
    }

    pub fn size(&self) -> [f32; 3] {
        std::array::from_fn(|i| self.max[i] - self.min[i])
    }

    fn overlaps(&self, o: &Bounds) -> bool {
        (0..3).all(|i| self.min[i] <= o.max[i] && o.min[i] <= self.max[i])
    }

    /// The bottom centre and the four bottom corners pulled in by a quarter.
    fn footprint(&self) -> [[f32; 2]; 5] {
        let c = self.center();
        let (qx, qy) = (self.size()[0] * 0.25, self.size()[1] * 0.25);
        [[c[0], c[1]], [c[0] - qx, c[1] - qy], [c[0] + qx, c[1] - qy], [c[0] - qx, c[1] + qy], [c[0] + qx, c[1] + qy]]
    }
}

/// One reference as the hunt sees it.
#[derive(Clone, Debug)]
pub struct Seen {
    pub form_id: u32,
    pub sig: [u8; 4],
    pub edid: String,
    pub model: String,
    pub drawn: Bounds,
    pub collision: Option<fnv_phys::RefBox>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Finding {
    pub kind: Kind,
    pub form_id: String,
    pub base_type: String,
    pub edid: String,
    pub model: String,
    pub at: [f32; 3],
    pub drawn: Bounds,
    /// The box round its collision, when it has any.
    pub collision: Option<Bounds>,
    pub detail: String,
}

impl Finding {
    fn of(s: &Seen, kind: Kind, detail: String) -> Finding {
        Finding {
            kind,
            form_id: format!("{:08X}", s.form_id),
            base_type: String::from_utf8_lossy(&s.sig).into_owned(),
            edid: s.edid.clone(),
            model: s.model.clone(),
            at: s.drawn.center(),
            drawn: s.drawn,
            collision: s.collision.as_ref().map(|c| Bounds { min: c.min, max: c.max }),
            detail,
        }
    }
}

/// What physics finds under points: for each, the gap from `z` down to the first thing
/// that is not the reference itself, and the terrain height there.
pub trait Ground {
    fn gap_below(&mut self, form_id: u32, x: f32, y: f32, z: f32, max: f32) -> Option<f32>;
    fn terrain_z(&mut self, x: f32, y: f32) -> Option<f32>;
}

fn is_solid_kind(sig: &[u8; 4]) -> bool {
    matches!(sig, b"STAT" | b"SCOL" | b"FURN" | b"CONT" | b"DOOR" | b"TERM" | b"ACTI")
}

/// Bushes, cacti, flowers and the like, which the game's data lets the player walk through
/// [observed 2026-11-21: in Goodsprings, about 160 of 198 missing-collision suspects].
pub fn is_plant(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    ["bush", "cactus", "yucca", "plant", "flower", "tree", "grass", "weed", "maize", "nettle", "shrub", "brush"].iter().any(|w| m.contains(w))
}

/// Signs, lights, glows, stains, banners and drapes: made to hang in the air or on a wall,
/// mostly without collision [observed 2026-11-21: Goodsprings, the rest of the suspects
/// and the floating statics after plants were left out].
pub fn hangs(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    ["effects\\", "sign", "light", "lamp", "neon", "stain", "decal", "banner", "drape", "window", "flag", "wire", "cable", "chain", "poleline"].iter().any(|w| m.contains(w))
}

/// Building-kit pieces (rails, ramps, pipes, decks), which join each other sideways rather
/// than stand on the ground, so "nothing under it" says nothing about them [observed
/// 2026-11-21: Megaton, 276 floating statics, all `architecture\megaton` and metro pieces].
pub fn is_kit_piece(model: &str) -> bool {
    let m = model.to_ascii_lowercase();
    m.contains("architecture\\") || m.contains("dungeons\\")
}

fn is_loose_kind(sig: &[u8; 4]) -> bool {
    matches!(sig, b"MISC" | b"WEAP" | b"ARMO" | b"AMMO" | b"ALCH" | b"BOOK" | b"KEYM" | b"NOTE" | b"CONT" | b"FURN" | b"CHIP" | b"CMNY" | b"IMOD")
}

/// The checks on one reference ([module notes](self)). `exterior`: terrain checks apply.
pub fn check(s: &Seen, exterior: bool, ground: &mut dyn Ground) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut say = |kind: Kind, detail: String| out.push(Finding::of(s, kind, detail));
    let d = &s.drawn;
    let size = d.size();
    let longest = size.iter().copied().fold(0.0, f32::max);
    match &s.collision {
        None => {
            if is_solid_kind(&s.sig) && longest >= SOLID_MIN_SIZE && size[2] >= 8.0 && !is_plant(&s.model) && !hangs(&s.model) {
                say(Kind::NoCollision, format!("model {:.0}x{:.0}x{:.0} units, no collision loaded", size[0], size[1], size[2]));
            }
        }
        Some(c) => {
            let cb = Bounds { min: c.min, max: c.max };
            let (cc, dc) = (cb.center(), d.center());
            let apart = ((cc[0] - dc[0]).powi(2) + (cc[1] - dc[1]).powi(2) + (cc[2] - dc[2]).powi(2)).sqrt();
            // Only for collision that is meant to be solid where the model is: invisible
            // walls, triggers and the like stand anywhere.
            // Doors are left out: a gate's drawn nodes can give a box of one piece only, and
            // collision doors are invisible by design [observed 2026-11-21: Megaton's main
            // gate drawn 36x40x0 against its 889-unit collision].
            let solid_layers = s.sig != *b"DOOR" && c.layers.iter().any(|l| !matches!(l, 12 | 15 | 17 | 27 | 31 | 34));
            if solid_layers && !cb.overlaps(d) {
                say(Kind::Misaligned, format!("collision box does not touch the model (centres {apart:.0} units apart)"));
            } else if solid_layers && apart > 0.5 * longest + 16.0 {
                say(Kind::Misaligned, format!("collision centre {apart:.0} units from the model's (model {longest:.0} units long)"));
            }
            let cs = cb.size();
            if let Some(i) = (0..3).find(|&i| s.sig != *b"DOOR" && cs[i] > 2.0 * size[i] + 64.0) {
                say(Kind::CollisionTooBig, format!("collision {:.0} units on {} against the model's {:.0}", cs[i], ["x", "y", "z"][i], size[i]));
            }
            if c.dynamic && exterior {
                if let Some(t) = ground.terrain_z(cc[0], cc[1]).filter(|t| c.max[2] < *t - 2.0) {
                    say(Kind::FellThrough, format!("collision top {:.0} under the terrain at {t:.0}", c.max[2]));
                }
            }
        }
    }
    // Floating loose things: nothing under the footprint within ITEM_GAP.
    let loose = is_loose_kind(&s.sig) || s.collision.as_ref().is_some_and(|c| c.dynamic);
    if loose {
        let gaps: Vec<Option<f32>> = d.footprint().iter().map(|p| rest_gap(ground, s.form_id, p[0], p[1], d.min[2], size[2])).collect();
        // A point whose ground is at or over the bottom (the base set into the ground, where
        // the ray from inside it sees no terrain) holds it up too.
        let held = d.footprint().iter().zip(&gaps).any(|(p, g)| g.is_some_and(|g| g - 1.0 <= ITEM_GAP) || ground.terrain_z(p[0], p[1]).is_some_and(|t| t >= d.min[2] - ITEM_GAP));
        // No terrain and nothing at all under it: its ground is not loaded, not a finding.
        let unknown = gaps.iter().all(Option::is_none) && ground.terrain_z(d.center()[0], d.center()[1]).is_none();
        if !held && !unknown {
            let least = gaps.iter().flatten().copied().fold(f32::MAX, f32::min);
            say(Kind::Floating, if least == f32::MAX { "nothing under it within 2000 units".into() } else { format!("{:.0} units over what is under it", least - 1.0) });
        }
    } else if exterior && s.sig == *b"STAT" && !hangs(&s.model) {
        let ts: Vec<f32> = d.footprint().iter().filter_map(|p| ground.terrain_z(p[0], p[1])).collect();
        if ts.len() == 5 {
            let (lo, hi) = (ts.iter().copied().fold(f32::MAX, f32::min), ts.iter().copied().fold(f32::MIN, f32::max));
            if d.min[2] > hi + STATIC_GAP && !is_kit_piece(&s.model) {
                let something = d.footprint().iter().any(|p| rest_gap(ground, s.form_id, p[0], p[1], d.min[2], size[2]).is_some_and(|g| g - 1.0 <= STATIC_GAP));
                if !something {
                    say(Kind::Floating, format!("bottom {:.0} units over the terrain", d.min[2] - hi));
                }
            }
            // Only wholly buried: roads and rocks set mostly into the ground are by design.
            if size[2] > 4.0 && d.max[2] < lo - SUNK_DEPTH {
                say(Kind::Sunk, format!("wholly under the terrain (top {:.0}, terrain {lo:.0})", d.max[2]));
            }
        }
    }
    out
}

/// How far under `bottom` the first thing below (x, y) is, plus 1 (as `gap_below` from
/// `bottom + 1`); 1 or less when the surface is at or over the bottom. The ray starts
/// inside the model (up to [`REST_PROBE`] over its bottom), so a thing set a few units
/// into a roof or shelf still finds it: from just over the bottom the ray started under
/// the surface and went on to the terrain [observed 2026-11-22: Goodsprings A403F909
/// wrench, bottom about 9,088, on the SCOLgsGasStation roof whose collision is at 9,096
/// (`scolgsgasstation.nif` Havok2, 46.64 Havok units over the reference at 8,770),
/// reported "327 units over what is under it"].
fn rest_gap(ground: &mut dyn Ground, form_id: u32, x: f32, y: f32, bottom: f32, height: f32) -> Option<f32> {
    // At least 16 even for a flat item: the wrench (3 units high) sits about 8 units into
    // the roof [observed 2026-11-22: still "327 units over" with the probe at its height].
    let lift = height.clamp(16.0, REST_PROBE);
    ground.gap_below(form_id, x, y, bottom + lift, 2000.0 + lift).map(|g| g - lift + 1.0)
}

/// Highest a floating check's ray starts over a model's bottom, game units [design].
pub const REST_PROBE: f32 = 32.0;

/// The interpenetration of dynamic bodies, from physics' last contacts.
fn penetrations(world: &fnv_phys::PhysWorld, s: &Seen) -> Option<String> {
    let deepest = world.ref_contacts(s.form_id).into_iter().filter(|(_, o, _)| !matches!(o, Some(Owner::Character { .. }) | Some(Owner::Actor { .. }))).min_by(|a, b| a.2.total_cmp(&b.2))?;
    (deepest.2 < -PENETRATION).then(|| {
        let with = match deepest.1 {
            Some(Owner::Terrain { .. }) => "the terrain".to_string(),
            Some(Owner::Ref { form_id, .. }) => format!("{form_id:08X}"),
            _ => "something".to_string(),
        };
        format!("overlaps {with} by {:.1} units", -deepest.2)
    })
}

struct PhysGround<'a> {
    q: fnv_phys::Queries<'a>,
}

impl Ground for PhysGround<'_> {
    fn gap_below(&mut self, form_id: u32, x: f32, y: f32, z: f32, max: f32) -> Option<f32> {
        let f = Filter { exclude: vec![Key::Ref(form_id)], no_actors: true, ..Default::default() };
        self.q.ray([x, y, z], [0.0, 0.0, -1.0], max, &f).map(|h| h.distance)
    }

    fn terrain_z(&mut self, x: f32, y: f32) -> Option<f32> {
        let f = Filter { no_actors: true, ..Default::default() };
        self.q.ray_all([x, y, 60000.0], [0.0, 0.0, -1.0], 120000.0, &f).into_iter().find(|h| matches!(h.owner, Owner::Terrain { .. })).map(|h| h.point[2])
    }
}

/// What the console asked for.
#[derive(Clone, Debug, PartialEq)]
pub enum Ask {
    /// Every loaded reference, or within this radius of the player.
    Around(Option<f32>),
    /// Round each open Bug Log note here.
    Notes(f32),
    Show(bool),
    /// One reference's drawn and collision boxes, and whether physics lists it.
    Ref(u32),
}

impl Ask {
    pub fn parse(rest: &str) -> Result<Ask, String> {
        let w: Vec<&str> = rest.split_whitespace().collect();
        let radius = |s: Option<&&str>, d: Option<f32>| match s {
            None => Ok(d),
            Some(v) => v.parse::<f32>().ok().filter(|r| r.is_finite() && *r > 0.0).map(Some).ok_or_else(|| format!("bughunt: {v} is not a radius")),
        };
        match w.first().map(|s| s.to_ascii_lowercase()).as_deref() {
            None => Ok(Ask::Around(None)),
            Some("notes") => radius(w.get(1), Some(NOTE_RADIUS)).map(|r| Ask::Notes(r.unwrap_or(NOTE_RADIUS))),
            Some("show") => Ok(Ask::Show(true)),
            Some("off") => Ok(Ask::Show(false)),
            Some("ref") => w.get(1).and_then(|v| u32::from_str_radix(v.trim_start_matches("0x"), 16).ok()).map(Ask::Ref).ok_or_else(|| "bughunt ref <FormID in hex>".to_string()),
            Some(_) => radius(w.first(), None).map(Ask::Around),
        }
    }
}

#[derive(Resource, Default)]
pub struct BugHunt {
    /// Asks from the console, one run a frame.
    pub asked: Vec<Ask>,
    pub findings: Vec<Finding>,
    pub show: bool,
    /// Lines for the console, taken by it.
    pub out: Vec<String>,
}

pub struct BugHuntPlugin;

impl Plugin for BugHuntPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BugHunt>().add_systems(Update, (hunt, draw.run_if(|h: Res<BugHunt>| h.show)).chain());
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn hunt(
    mut h: ResMut<BugHunt>,
    mut phys: ResMut<PhysState>,
    play: Res<PlayWorld>,
    loc: Option<Res<Location>>,
    player: Option<Res<crate::player::Player>>,
    shapes: Query<(&RefVisual, &Aabb, &GlobalTransform, &InheritedVisibility), Without<bevy::mesh::skinning::SkinnedMesh>>,
) {
    if h.asked.is_empty() {
        return;
    }
    let ask = h.asked.remove(0);
    let (centres, radius): (Vec<[f32; 3]>, Option<f32>) = match ask {
        Ask::Show(on) => {
            h.show = on;
            h.out.push(format!("Bug hunt boxes {}", if on { "shown" } else { "hidden" }));
            return;
        }
        Ask::Ref(id) => {
            let mut b = Bounds::EMPTY;
            let mut shown = 0;
            for (_, aabb, gt, vis) in shapes.iter().filter(|(r, ..)| r.0 == id) {
                shown += usize::from(vis.get());
                let (c, e) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
                for i in 0..8 {
                    b.add(space::to_game(gt.transform_point(c + Vec3::new(if i & 1 == 0 { -e.x } else { e.x }, if i & 2 == 0 { -e.y } else { e.y }, if i & 4 == 0 { -e.z } else { e.z }))));
                }
            }
            let f = |v: [f32; 3]| format!("{:.0}, {:.0}, {:.0}", v[0], v[1], v[2]);
            let coll = phys.world.ref_boxes().into_iter().find(|c| c.form_id == id);
            h.out.push(format!("{id:08X}: physics lists it: {}; drawn shapes {shown}", phys.ref_cell.contains_key(&id)));
            h.out.push(if b.is_empty() { "  not drawn".into() } else { format!("  drawn ({}) .. ({})", f(b.min), f(b.max)) });
            h.out.push(match coll {
                Some(c) => format!("  collision ({}) .. ({}), dynamic {}, layers {:?}", f(c.min), f(c.max), c.dynamic, c.layers),
                None => "  no collision".into(),
            });
            return;
        }
        Ask::Around(r) => (player.as_ref().map(|p| vec![p.feet]).unwrap_or_default(), r),
        Ask::Notes(r) => {
            let here = loc.as_deref().map(|l| (l.world, l.interior));
            let notes = crate::bug_log::path().map(|p| crate::bug_log::load(&p)).unwrap_or_default();
            let (mut near, mut away) = (Vec::new(), 0);
            for n in notes.iter().filter(|n| n.status == "open") {
                let same = here.is_some_and(|(w, i)| if i.is_some() { n.interior == i } else { n.interior.is_none() && n.world == w });
                if same {
                    near.push(n.pos);
                } else {
                    away += 1;
                }
            }
            h.out.push(format!("bughunt notes: {} open notes here, {away} elsewhere (go there and run it again)", near.len()));
            if near.is_empty() {
                return;
            }
            (near, Some(r))
        }
    };
    let exterior = loc.as_deref().is_some_and(|l| l.interior.is_none());
    let t = std::time::Instant::now();
    // The drawn boxes of every placed reference physics loaded.
    let mut drawn: HashMap<u32, Bounds> = HashMap::new();
    for (r, aabb, gt, vis) in &shapes {
        if !vis.get() {
            continue;
        }
        let b = drawn.entry(r.0).or_insert(Bounds::EMPTY);
        let (c, e) = (Vec3::from(aabb.center), Vec3::from(aabb.half_extents));
        for i in 0..8 {
            let corner = c + Vec3::new(if i & 1 == 0 { -e.x } else { e.x }, if i & 2 == 0 { -e.y } else { e.y }, if i & 4 == 0 { -e.z } else { e.z });
            b.add(space::to_game(gt.transform_point(corner)));
        }
    }
    let within = |p: [f32; 3]| radius.is_none_or(|r| centres.iter().any(|c| (c[0] - p[0]).powi(2) + (c[1] - p[1]).powi(2) + (c[2] - p[2]).powi(2) <= r * r));
    let boxes: HashMap<u32, fnv_phys::RefBox> = phys.world.ref_boxes().into_iter().map(|b| (b.form_id, b)).collect();
    let actors: std::collections::HashSet<u32> = phys.npcs.keys().copied().collect();
    let feet = player.as_ref().map(|p| p.feet);
    let near_player = |p: [f32; 3]| feet.is_some_and(|f| (f[0] - p[0]).powi(2) + (f[1] - p[1]).powi(2) <= PHYS_REACH * PHYS_REACH);
    let (mut seen, mut unlisted) = (Vec::new(), Vec::new());
    for (&id, b) in &drawn {
        if b.is_empty() || actors.contains(&id) || !within(b.center()) {
            continue;
        }
        let Some(p) = fnv_world::placed::placement(play.0.lo, play.0.schema, id).filter(|p| &p.sig == b"REFR") else { continue };
        let Some(base) = p.base.and_then(|f| fnv_world::placed::base_object(play.0.lo, play.0.schema, f)) else { continue };
        if matches!(&base.sig, b"LIGH" | b"TREE" | b"GRAS" | b"PWAT" | b"IDLM" | b"TXST") {
            continue;
        }
        let s = Seen { form_id: id, sig: base.sig, edid: base.edid.clone().unwrap_or_default(), model: base.world_model_path().unwrap_or_default(), drawn: *b, collision: boxes.get(&id).cloned() };
        if phys.ref_cell.contains_key(&id) {
            seen.push(s);
        } else if near_player(b.center()) && is_solid_kind(&s.sig) && b.size().iter().any(|v| *v >= SOLID_MIN_SIZE) && !is_plant(&s.model) && !hangs(&s.model) {
            // Drawn next to the player, but physics never listed it: no collision at all,
            // whatever its model has.
            unlisted.push(Finding::of(&s, Kind::NotInPhysics, "drawn here, but physics does not list it among its loaded cells' references".into()));
        }
    }
    seen.sort_by_key(|s| s.form_id);
    let mut findings = unlisted;
    for s in &seen {
        if s.collision.as_ref().is_some_and(|c| c.dynamic) {
            if let Some(detail) = penetrations(&phys.world, s) {
                findings.push(Finding::of(s, Kind::Interpenetrating, detail));
            }
        }
    }
    {
        let mut g = PhysGround { q: phys.world.queries() };
        for s in &seen {
            findings.extend(check(s, exterior, &mut g));
        }
    }
    findings.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.form_id.cmp(&b.form_id)));
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let place = loc.as_deref().map_or("?".to_string(), |l| l.interior.map_or(format!("worldspace {:08X}", l.world), |i| format!("interior {i:08X}")));
    let counts: Vec<String> = Kind::ALL.iter().map(|k| format!("{k:?} {}", findings.iter().filter(|f| f.kind == *k).count())).collect();
    let summary = format!("bug hunt: {} references checked in {place} ({:.0} ms): {} findings: {}", seen.len(), ms, findings.len(), counts.join(", "));
    info!("{summary}");
    h.out.push(summary);
    match write_report(&place, seen.len(), &findings) {
        Ok(p) => h.out.push(format!("Report: {}", p.display())),
        Err(e) => h.out.push(format!("bug hunt: report not written: {e}")),
    }
    for f in findings.iter().take(8) {
        h.out.push(format!("{:?} {} {} {}: {}", f.kind, f.form_id, f.base_type, f.edid, f.detail));
    }
    h.findings = findings;
    h.show = true;
}

/// `<home>\bug-hunt\hunt-<unix s>.md` and `.json`; the path of the `.md`.
fn write_report(place: &str, checked: usize, findings: &[Finding]) -> Result<std::path::PathBuf, String> {
    let dir = crate::bug_log::path().and_then(|p| p.parent().map(|d| d.join("bug-hunt"))).ok_or("no home folder")?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let md = dir.join(format!("hunt-{now}.md"));
    fnv_install::guard_write(&md, None).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(md.with_extension("json"), serde_json::to_string_pretty(findings).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    std::fs::write(&md, report_text(place, checked, findings, now)).map_err(|e| e.to_string())?;
    Ok(md)
}

pub fn report_text(place: &str, checked: usize, findings: &[Finding], unix: u64) -> String {
    let mut s = format!("# Bug hunt, {place}\n\nUnix time {unix}; {checked} references checked; {} findings.\n", findings.len());
    for k in Kind::ALL {
        let of: Vec<&Finding> = findings.iter().filter(|f| f.kind == k).collect();
        if of.is_empty() {
            continue;
        }
        s += &format!("\n## {k:?} ({})\n\n| Ref | Type | Editor ID | At (x, y, z) | Detail | Model |\n|---|---|---|---|---|---|\n", of.len());
        for f in of {
            s += &format!("| {} | {} | {} | {:.0}, {:.0}, {:.0} | {} | {} |\n", f.form_id, f.base_type, f.edid, f.at[0], f.at[1], f.at[2], f.detail, f.model);
        }
    }
    s
}

const EDGES: [(usize, usize); 12] = [(0, 1), (2, 3), (4, 5), (6, 7), (0, 2), (1, 3), (4, 6), (5, 7), (0, 4), (1, 5), (2, 6), (3, 7)];

fn draw(h: Res<BugHunt>, mut gizmos: Gizmos) {
    for f in &h.findings {
        let b = &f.drawn;
        let p: [Vec3; 8] = std::array::from_fn(|i| space::to_bevy([if i & 1 == 0 { b.min[0] } else { b.max[0] }, if i & 2 == 0 { b.min[1] } else { b.max[1] }, if i & 4 == 0 { b.min[2] } else { b.max[2] }]));
        for (a, c) in EDGES {
            gizmos.line(p[a], p[c], f.kind.colour());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Flat terrain at z 0 and, optionally, a shelf at `shelf` under every point.
    struct Flat {
        shelf: Option<f32>,
    }

    impl Ground for Flat {
        fn gap_below(&mut self, _: u32, _: f32, _: f32, z: f32, max: f32) -> Option<f32> {
            let top = self.shelf.filter(|s| *s <= z).unwrap_or(0.0);
            (z - top <= max && z >= top).then_some(z - top)
        }
        fn terrain_z(&mut self, _: f32, _: f32) -> Option<f32> {
            Some(0.0)
        }
    }

    fn seen(sig: &[u8; 4], min: [f32; 3], max: [f32; 3], collision: Option<([f32; 3], [f32; 3], bool)>) -> Seen {
        Seen {
            form_id: 0x1234,
            sig: *sig,
            edid: "Test".into(),
            model: "test.nif".into(),
            drawn: Bounds { min, max },
            collision: collision.map(|(min, max, dynamic)| fnv_phys::RefBox { form_id: 0x1234, min, max, dynamic, layers: vec![1] }),
        }
    }

    fn kinds(s: &Seen, shelf: Option<f32>) -> Vec<Kind> {
        check(s, true, &mut Flat { shelf }).into_iter().map(|f| f.kind).collect()
    }

    #[test]
    fn a_wall_on_the_ground_with_its_collision_is_fine() {
        assert!(kinds(&seen(b"STAT", [0.0, 0.0, -2.0], [200.0, 20.0, 150.0], Some(([0.0, 0.0, -2.0], [200.0, 20.0, 150.0], false))), None).is_empty());
    }

    #[test]
    fn a_big_static_without_collision_is_a_suspect_and_small_clutter_is_not() {
        assert_eq!(kinds(&seen(b"STAT", [0.0, 0.0, 0.0], [200.0, 20.0, 150.0], None), None), vec![Kind::NoCollision]);
        assert!(kinds(&seen(b"STAT", [0.0, 0.0, 0.0], [20.0, 20.0, 10.0], None), None).is_empty());
    }

    #[test]
    fn collision_away_from_or_far_larger_than_the_model() {
        let k = kinds(&seen(b"STAT", [0.0, 0.0, 0.0], [100.0, 100.0, 100.0], Some(([500.0, 0.0, 0.0], [600.0, 100.0, 100.0], false))), None);
        assert_eq!(k, vec![Kind::Misaligned]);
        // The x7 strips: a 100-unit railing with 700 units of collision.
        let k = kinds(&seen(b"STAT", [0.0, 0.0, 0.0], [100.0, 10.0, 40.0], Some(([0.0, 0.0, 0.0], [700.0, 70.0, 280.0], false))), None);
        assert!(k.contains(&Kind::CollisionTooBig), "{k:?}");
    }

    #[test]
    fn loose_items_float_unless_something_is_under_them() {
        let can = |z: f32| seen(b"MISC", [0.0, 0.0, z], [5.0, 5.0, z + 8.0], Some(([0.0, 0.0, z], [5.0, 5.0, z + 8.0], true)));
        assert!(kinds(&can(0.5), None).is_empty());
        assert_eq!(kinds(&can(40.0), None), vec![Kind::Floating]);
        // On a table top at 39.
        assert!(kinds(&can(40.0), Some(39.0)).is_empty());
        // Set 6 units into a roof at 46: the ray from inside the can still finds the roof.
        assert!(kinds(&can(40.0), Some(46.0)).is_empty());
    }

    #[test]
    fn statics_float_over_or_sink_into_the_terrain() {
        assert_eq!(kinds(&seen(b"STAT", [0.0, 0.0, 100.0], [100.0, 100.0, 200.0], Some(([0.0, 0.0, 100.0], [100.0, 100.0, 200.0], false))), None), vec![Kind::Floating]);
        assert_eq!(kinds(&seen(b"STAT", [0.0, 0.0, -300.0], [100.0, 100.0, -200.0], Some(([0.0, 0.0, -300.0], [100.0, 100.0, -200.0], false))), None), vec![Kind::Sunk]);
        // A road flush with the ground is fine.
        assert!(kinds(&seen(b"STAT", [0.0, 0.0, -30.0], [100.0, 100.0, 1.0], Some(([0.0, 0.0, -30.0], [100.0, 100.0, 1.0], false))), None).is_empty());
        // Plants are not missing collision.
        let mut bush = seen(b"STAT", [0.0, 0.0, 0.0], [200.0, 200.0, 150.0], None);
        bush.model = r"meshes\clutter\creosotebush\creosotebushlg.nif".into();
        assert!(kinds(&bush, None).is_empty());
        // A walkway piece high over the ground joins its neighbours sideways.
        let mut rail = seen(b"STAT", [0.0, 0.0, 300.0], [100.0, 10.0, 340.0], Some(([0.0, 0.0, 300.0], [100.0, 10.0, 340.0], false)));
        rail.model = r"meshes\architecture\megaton\megatondeckrailstraightreg.nif".into();
        assert!(kinds(&rail, None).is_empty());
        // A rock mostly above the ground is fine.
        assert!(kinds(&seen(b"STAT", [0.0, 0.0, -30.0], [100.0, 100.0, 70.0], Some(([0.0, 0.0, -30.0], [100.0, 100.0, 70.0], false))), None).is_empty());
    }

    #[test]
    fn a_dynamic_body_under_the_terrain_fell_through() {
        let k = kinds(&seen(b"MISC", [0.0, 0.0, -50.0], [5.0, 5.0, -40.0], Some(([0.0, 0.0, -50.0], [5.0, 5.0, -40.0], true))), None);
        assert!(k.contains(&Kind::FellThrough), "{k:?}");
    }

    #[test]
    fn console_arguments() {
        assert_eq!(Ask::parse(""), Ok(Ask::Around(None)));
        assert_eq!(Ask::parse("1500"), Ok(Ask::Around(Some(1500.0))));
        assert_eq!(Ask::parse("notes"), Ok(Ask::Notes(NOTE_RADIUS)));
        assert_eq!(Ask::parse("NOTES 300"), Ok(Ask::Notes(300.0)));
        assert_eq!(Ask::parse("off"), Ok(Ask::Show(false)));
        assert_eq!(Ask::parse("ref 0017748C"), Ok(Ask::Ref(0x0017748C)));
        assert!(Ask::parse("ref").is_err());
        assert!(Ask::parse("far").is_err());
    }

    #[test]
    fn the_report_lists_findings_by_kind() {
        let s = seen(b"STAT", [0.0, 0.0, 0.0], [200.0, 20.0, 150.0], None);
        let f = check(&s, true, &mut Flat { shelf: None });
        let text = report_text("worldspace 00000001", 1, &f, 0);
        assert!(text.contains("## NoCollision (1)") && text.contains("| 00001234 | STAT | Test |"), "{text}");
    }
}
