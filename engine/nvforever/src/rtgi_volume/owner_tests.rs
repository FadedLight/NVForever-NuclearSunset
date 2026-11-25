//! Who may have a volume and when, without a GPU: the plan as a table, and that nothing is done without a request.

use super::gpu::RtgiGpu;
use super::plan::{plan, Admission, Key, Latch, Now, Pipelines, Plan, Why};
use super::tests::{extracted, noop};
use super::*;
use crate::world::Streamer;
use bevy::app::SubApp;
use bevy::pbr::{check_views_need_specialization, MeshPipelineKey, NvfRtgiVolumeBindings, ViewKeyCache};
use bevy::render::camera::DirtySpecializations;
use bevy::render::render_resource::CachedPipelineState;
use bevy::render::sync_world::{MainEntity, RenderEntity};
use bevy::render::view::RetainedViewEntity;
use bevy::render::{ExtractSchedule, MainWorld, RenderStartup};
use bevy::shader::ShaderCacheError;

const HERE: Place = Place { cell: 0x0010_4C1A, travels: 3, game: Some(7) };

fn key(place: Place, revision: u64, solved: bool) -> Key {
    Key { place, revision, solved }
}

// ---------------------------------------------------------------------------------------
// The plan.

/// All there is to a request, one thing changed at a time.
#[test]
fn the_plan() {
    let now = Now { here: Some(HERE), hook: HookState::Installed, admission: Admission::Granted, open: true, failed: None };
    let shut = Now { open: false, ..now };
    let (empty, solved) = (key(HERE, 1, false), key(HERE, 1, true));
    let stock = Plan::Stock;
    let elsewhere = [Place { cell: 1, ..HERE }, Place { travels: 2, ..HERE }, Place { game: None, ..HERE }, Place { game: Some(6), ..HERE }];

    // The first binding, under the screen alone; the rest of a visit at any time.
    for want in [empty, solved] {
        assert_eq!(plan(Some(want), &now, None), Plan::Bind { new: true });
        assert_eq!(plan(Some(want), &shut, None), stock(Why::Late));
        assert_eq!(plan(Some(want), &shut, Some(want)), Plan::Bind { new: false });
    }
    assert_eq!(plan(Some(solved), &shut, Some(empty)), Plan::Bind { new: true }, "empty to solved");
    assert_eq!(plan(Some(key(HERE, 2, true)), &shut, Some(solved)), Plan::Bind { new: true }, "the next revision");
    assert_eq!(plan(Some(solved), &shut, Some(key(HERE, 2, true))), Plan::Keep, "an older result is dropped");

    // No request, or one of another place: nothing stays, under a screen or not.
    for now in [now, shut] {
        assert_eq!(plan(None, &now, Some(solved)), stock(Why::Nothing));
        for other in elsewhere {
            assert_eq!(plan(Some(key(other, 9, true)), &now, None), stock(Why::Elsewhere), "a stale result");
            assert_eq!(plan(Some(key(other, 9, true)), &now, Some(key(other, 9, true))), stock(Why::Elsewhere), "a stale one that is bound");
        }
        assert_eq!(plan(Some(solved), &Now { here: None, ..now }, Some(solved)), stock(Why::Elsewhere), "outdoors");
    }
    // What was bound elsewhere gives no right here.
    for other in elsewhere {
        assert_eq!(plan(Some(solved), &shut, Some(key(other, 1, true))), stock(Why::Late));
        assert_eq!(plan(Some(solved), &now, Some(key(other, 1, true))), Plan::Bind { new: true });
    }
    // The hook, the device, a pipeline: none is bound, and what is bound goes.
    for held in [None, Some(solved)] {
        for (why, now) in [
            (Why::Waiting, Now { hook: HookState::Pending, ..now }),
            (Why::Hook, Now { hook: HookState::Failed, ..now }),
            (Why::Waiting, Now { admission: Admission::Deferred, ..now }),
            (Why::Refused, Now { admission: Admission::Refused, ..now }),
            (Why::Failed, Now { failed: Some(HERE), ..now }),
        ] {
            assert_eq!(plan(Some(solved), &now, held), stock(why), "{held:?}");
        }
        assert!(matches!(plan(Some(solved), &Now { failed: Some(elsewhere[1]), ..now }, held), Plan::Bind { .. }), "failed in another visit");
    }
}

/// Open from a screen's first frame until it settles, and not again under the same screen.
#[test]
fn the_latch() {
    let mut latch = Latch::default();
    let frames = [(false, false), (true, false), (true, false), (true, true), (true, false), (false, false), (true, false), (true, true), (false, false)];
    assert_eq!(frames.map(|(screen, settling)| latch.step(screen, settling)), [false, true, true, false, false, false, true, false, false]);
}

/// An error that is tried again is no failure; any other is.
#[test]
fn a_reader_fails_for_good_only() {
    let mut n = Pipelines::default();
    for e in [ShaderCacheError::ShaderImportNotYetAvailable, ShaderCacheError::ShaderNotLoaded(AssetId::default())] {
        n.count(&CachedPipelineState::Err(e));
    }
    n.count(&CachedPipelineState::Queued);
    assert_eq!(n, Pipelines { waiting: 3, ..default() });
    for e in [ShaderCacheError::CreateShaderModule("made".into()), ShaderCacheError::ProcessShaderError("made".into())] {
        n.count(&CachedPipelineState::Err(e));
    }
    assert_eq!(n, Pipelines { waiting: 3, failed: 2, ..default() });
}

// ---------------------------------------------------------------------------------------
// What is done.

fn streamer(interior: Option<u32>) -> Streamer {
    Streamer {
        radius: 2,
        loaded: default(),
        fades: default(),
        fade_secs: 0.0,
        settled: true,
        arrival: None,
        per_frame: 1,
        spawning: default(),
        dying: default(),
        ahead: default(),
        hide: default(),
        interior,
        light_scale: 1.0,
        managed: default(),
        manage_actors: false,
        stats: default(),
        buffer: default(),
        centre: None,
        waiting: default(),
        model_overrides: default(),
        travels: 0,
    }
}

/// Two travels into one cell are two places, outdoors is none.
#[test]
fn a_place_is_a_visit() {
    let mut s = streamer(Some(5));
    let first = place(Some(&s), None);
    assert_eq!(first, Some(Place { cell: 5, travels: 0, game: None }));
    s.travels += 1;
    assert_ne!(place(Some(&s), None), first);
    s.interior = None;
    assert_eq!((place(Some(&s), None), place(None, None)), (None, None));
}

/// One extraction, as Bevy's own: the main world lent to the render world.
fn extraction(app: &mut App) {
    let mut render = app.remove_sub_app(RenderApp).unwrap();
    let mut main = MainWorld::default();
    std::mem::swap(&mut *main, app.world_mut());
    render.world_mut().insert_resource(main);
    render.world_mut().run_schedule(ExtractSchedule);
    let mut main = render.world_mut().remove_resource::<MainWorld>().unwrap();
    std::mem::swap(&mut *main, app.world_mut());
    app.insert_sub_app(RenderApp, render);
}

/// Two views in a world, and their keys.
fn views(world: &mut World) -> [(Entity, RetainedViewEntity); 2] {
    [Msaa::Off, Msaa::Sample4].map(|msaa| {
        let view = RetainedViewEntity::new(MainEntity::from(world.spawn_empty().id()), None, 0);
        (world.spawn((extracted(view), msaa)).id(), view)
    })
}

fn keys(world: &mut World, views: &[(Entity, RetainedViewEntity); 2]) -> [MeshPipelineKey; 2] {
    world.init_resource::<ViewKeyCache>();
    world.init_resource::<DirtySpecializations>();
    world.run_system_cached(check_views_need_specialization).unwrap();
    views.map(|(_, view)| world.resource::<ViewKeyCache>()[&view])
}

/// The plugin with nothing asked for, two cameras, an interior, a travel and the outdoors, in worlds
/// without a device or a queue (what would make something panics): nothing is counted, held or bound,
/// and the views' keys are those without the plugin. Then a request, with no device to ask: not yet.
#[test]
fn nothing_is_done_without_a_request() {
    let mut plain = World::new();
    let of_plain = views(&mut plain);
    let before = keys(&mut plain, &of_plain);

    let mut app = App::new();
    app.init_resource::<Assets<Shader>>().insert_sub_app(RenderApp, SubApp::new());
    app.add_plugins(RtgiVolumePlugin);
    let render = app.sub_app_mut(RenderApp).world_mut();
    render.run_schedule(RenderStartup);
    let ours = views(render);
    for (view, _) in ours {
        app.world_mut().spawn((Camera3d::default(), crate::fly::FlyCam { yaw: 0.0, pitch: 0.0, speed: 0.0 }, RenderEntity::from(view)));
    }
    app.init_resource::<nvf_play::loading::Loading>().insert_resource(streamer(Some(HERE.cell)));
    let check = |app: &mut App, case: &str| {
        extraction(app);
        let render = app.sub_app_mut(RenderApp).world_mut();
        assert_eq!(render.resource::<RtgiCounts>().all(), [0; 4], "{case}");
        let state = render.resource::<RtgiGpu>();
        assert!(state.held.is_none(), "{case}");
        assert_eq!(render.query::<&NvfRtgiVolumeBindings>().iter(render).count(), 0, "{case}");
        assert_eq!(keys(render, &ours), before, "{case}");
    };
    check(&mut app, "an interior");
    app.world_mut().resource_mut::<Streamer>().travels += 1;
    check(&mut app, "a travel");
    app.insert_resource(streamer(None));
    check(&mut app, "outdoors");
    assert!(!app.world().contains_resource::<RtgiRequest>());
    let there = streamer(Some(HERE.cell));
    let here = place(Some(&there), None).unwrap();
    app.insert_resource(there).insert_resource(RtgiRequest::empty(here, 1));
    check(&mut app, "a request before there is a device");
    // The same view with the component has another key, so that the comparison can fail.
    let render = app.sub_app_mut(RenderApp).world_mut();
    render.entity_mut(ours[0].0).insert(super::tests::bindings_on(&noop().0));
    assert_ne!(keys(render, &ours), before);
}
