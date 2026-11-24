//! Bevy's two-phase GPU occlusion culling on the main camera, as an experiment behind
//! `NVF_OCCLUSION_CULLING=1` (off by default; graphics pass 2026-11-23).
//!
//! The camera already has a depth prepass, which is all Bevy asks for. What it needs here:
//! first-person shapes (the arms, the held weapon, the seen body) write a depth squeezed
//! into [63/64, 1] that their bounding boxes do not describe (docs/design/first-person-depth.md),
//! so one whose box lies behind a wall would be culled though it is drawn in front of it.
//! Those shapes keep `NoFrustumCulling`, which the vendored mesh_preprocess.wesl also reads
//! as "never occlusion culled" (engine/vendor/README.md).

use bevy::camera::visibility::{NoFrustumCulling, RenderLayers};
use bevy::prelude::*;

/// Whether `NVF_OCCLUSION_CULLING=1` asks for the culling.
pub fn wanted() -> bool {
    std::env::var("NVF_OCCLUSION_CULLING").is_ok_and(|v| v == "1")
}

pub struct GpuOcclusionPlugin;

impl Plugin for GpuOcclusionPlugin {
    fn build(&self, app: &mut App) {
        if !wanted() {
            return;
        }
        info!("occlusion culling: on for the main camera (NVF_OCCLUSION_CULLING=1)");
        app.add_systems(PostUpdate, (add_to_main_camera, keep_first_person_unculled));
    }
}

/// The main camera (the fly camera, which has the depth prepass) without the culling yet.
type UnculledCamera = (With<crate::fly::FlyCam>, With<Camera3d>, Without<bevy::render::occlusion_culling::OcclusionCulling>);

/// Meshes that can still be frustum (and so occlusion) culled.
type CulledMesh = (With<Mesh3d>, Without<NoFrustumCulling>);

/// The main camera gets the culling once.
fn add_to_main_camera(mut commands: Commands, cams: Query<Entity, UnculledCamera>) {
    for e in &cams {
        commands.entity(e).insert(bevy::render::occlusion_culling::OcclusionCulling);
    }
}

/// Every mesh on the first-person layers keeps `NoFrustumCulling` (re-added when the
/// warm-pose code takes its own off, player_body.rs `WarmUnculled`).
fn keep_first_person_unculled(mut commands: Commands, meshes: Query<(Entity, &RenderLayers), CulledMesh>) {
    let first_person = RenderLayers::from_layers(&[crate::sky::ARMS_LAYER, crate::sky::BODY_LAYER]);
    for (e, layers) in &meshes {
        if layers.intersects(&first_person) {
            commands.entity(e).try_insert(NoFrustumCulling);
        }
    }
}
