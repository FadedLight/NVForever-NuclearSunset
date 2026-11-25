//! The ray scene of a cell: its triangles and what a ray that light could take meets among them. It is the
//! first part of a solver for a cell's bounce light (#3009, `docs/design/rtgi-solver.md`). Plain data in: no
//! Bevy, no files, no GPU, no state between calls. The same input gives the same tree and the same hits on
//! any number of threads.
//!
//! [`prepare`] makes the scene once; `bvh`, `texture` and `trace` are its parts.
//!
//! Space is Bevy's world in game units: y up, -z north. Colours are linear unless called display values.
//!
//! A floor, and what a ray from above it meets:
//!
//! ```
//! use nvf_rtgi::*;
//!
//! let scene = SceneInput {
//!     meshes: vec![Mesh {
//!         positions: vec![[-100.0, 0.0, 100.0], [100.0, 0.0, 100.0], [100.0, 0.0, -100.0], [-100.0, 0.0, -100.0]],
//!         indices: vec![0, 1, 2, 0, 2, 3],
//!         ..Mesh::default()
//!     }],
//!     instances: vec![Instance { mesh: 0, material: 0, world_from_local: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]] }],
//!     materials: vec![Material { base: [0.6, 0.5, 0.4, 1.0], ..Material::default() }],
//!     textures: Vec::new(),
//! };
//! let scene = prepare(scene, &SceneLimits::default(), 1, &|| false)?;
//! assert!(scene.stats().sound());
//!
//! let eye = V3::new(10.0, 64.0, 20.0);
//! let hit = scene.transport(&Ray::new(eye, V3::new(0.0, -1.0, 0.0)), f32::INFINITY).unwrap();
//! assert!(hit.front && !hit.hidden && hit.p.0[1].abs() < 1e-4);
//! assert!(hit.facing().0[1] > 0.999);
//! assert!((hit.albedo(Lookup::Color)[0] - 0.6).abs() < 1e-5);
//! assert!(scene.sees(hit.p, hit.facing(), eye));
//! # Ok::<(), Failed>(())
//! ```

#![forbid(unsafe_code)]

mod bvh;
mod failed;
mod scene;
mod texture;
mod trace;
mod vec3;

pub use bvh::Ray;
pub use failed::{Cancel, Failed, Reason};
pub use scene::{Cull, Instance, Level, Material, Mesh, SceneInput, Texture, Wrap};
pub use texture::Lookup;
pub use trace::{lift, Hit, PreparedScene, SceneLimits, SceneStats, MOST_THREADS};
pub use vec3::V3;

/// The ray scene of a cell. What is not sound is left out or replaced, and counted ([`SceneStats`]). `cancel`
/// is asked between pieces of the work, of a large tree on its threads too.
pub fn prepare(scene: SceneInput, limits: &SceneLimits, threads: usize, cancel: Cancel) -> Result<PreparedScene, Failed> {
    failed::guarded(|| PreparedScene::of(scene, limits, threads, cancel))
}

#[cfg(test)]
mod tests {
    use super::*;
    use trace::tests::{quad, scene, SAME};

    /// A panic of the caller's check is a failure, not the caller's end.
    #[test]
    fn panics_do_not_get_out() {
        let input = || scene(vec![(quad(0.0), SAME, Material::default())], Vec::new());
        let check = || -> bool { panic!("expected here") };
        assert_eq!(prepare(input(), &SceneLimits::default(), 1, &check).err(), Some(Failed::Internal));
        assert!(prepare(input(), &SceneLimits::default(), 1, &|| false).is_ok_and(|w| w.stats().sound()));
    }
}
