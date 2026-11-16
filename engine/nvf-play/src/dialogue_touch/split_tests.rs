//! Real conversation identity, camera ownership, production Taffy draw and raw touch.
use super::*;
use super::compact_tests::{layout_scene,rows,send,settle};
use crate::dialogue_split::{Options,Side};
use crate::touch_safe_area::SafeInsets;
use bevy::camera::{ComputedCameraValues,RenderTargetInfo};

fn publish_window_targets(windows:Query<&Window,With<PrimaryWindow>>,mut cameras:Query<&mut Camera>) {
    let w=windows.single().unwrap();
    for mut c in &mut cameras {
        c.computed=ComputedCameraValues { target_info:Some(RenderTargetInfo {physical_size:UVec2::new(w.physical_width(),w.physical_height()),scale_factor:w.scale_factor()}),..default() };
    }
}
fn scene(side:Side,density:f32)->(App,Entity,Entity) {
    let (mut app,window)=layout_scene(true,Vec2::new(792.0,363.0),density);
    app.configure_sets(Update,(crate::PlaySet::Input,crate::PlaySet::Physics,crate::PlaySet::Activation,crate::PlaySet::Scripts,crate::PlaySet::Ui,crate::PlaySet::Sync).chain());
    let mut script=crate::script::ScriptState::over_no_plugins();
    script.rt.as_mut().unwrap().game.dialogue=Some(fnv_game::dialogue::Conversation {
        actor:fnv_sim::FormId(42),through:None,line:None,choices:vec![],topic_list:false,lines:1,then:None,ending:false,
    });
    app.insert_resource(script)
        .insert_resource(crate::dialogue_look::SpeakerFace(Some((42,[35.0,180.0,130.0]))))
        .insert_resource(Options {side})
        .insert_resource(SafeInsets {left:24.0,top:12.0,right:10.0,bottom:18.0})
        .add_systems(Update,publish_window_targets.after(crate::PlaySet::Sync));
    let world=app.world_mut().spawn((
        crate::PlayerCamera,Camera3d::default(),Camera {order:1,..default()},
        Transform::from_xyz(0.0,112.0,0.0),
    )).id();
    settle(&mut app);
    (app,window,world)
}
fn panel(app:&mut App)->Rect {
    let mut q=app.world_mut().query_filtered::<(&ComputedNode,&UiGlobalTransform),With<Root>>();
    let (n,t)=q.single(app.world()).unwrap();
    Rect::from_center_size(t.transform_point2(Vec2::ZERO),n.size())
}
fn default_camera(app:&mut App)->Option<Entity> {
    let mut state=bevy::ecs::system::SystemState::<bevy::ui::DefaultUiCamera>::new(app.world_mut());
    state.get(app.world()).expect("default UI camera query must be valid").get()
}
#[test]
fn full_world_projects_speaker_opposite_measured_choices_at_both_densities() {
    for side in [Side::Left,Side::Right] {
        for density in [1.0,2.0] {
            let (mut app,_,world)=scene(side,density);
            assert!(app.world().get::<Camera>(world).unwrap().viewport.is_none(),"normal world viewport remains full-size");
            let choices=panel(&mut app);
            assert!(choices.min.x>=24.0*density && choices.min.y>=12.0*density);
            assert!(choices.max.x<=782.0*density && choices.max.y<=345.0*density);
            let camera=app.world().get::<Transform>(world).unwrap();
            let face=crate::space::to_bevy([35.0,180.0,130.0]);
            let view=camera.rotation.inverse()*(face-camera.translation);
            let Projection::Perspective(projection)=app.world().get::<Projection>(world).unwrap() else {panic!("perspective")};
            let x=(0.5+0.5*view.x/(-view.z*(projection.fov*0.5).tan()*(792.0/363.0)))*792.0*density;
            assert!(view.z<0.0);
            if side==Side::Left {assert!(x<choices.min.x && x>24.0*density);}else{assert!(x>choices.max.x && x<782.0*density);}
            let root=app.world_mut().query_filtered::<(Entity,&UiTargetCamera),With<Root>>().single(app.world()).unwrap().1.0;
            assert_ne!(root,world);
            assert!(app.world().get::<Camera>(root).unwrap().order>app.world().get::<Camera>(world).unwrap().order);
            assert!(app.world().get::<bevy::ui::IsDefaultUiCamera>(root).is_none());
            assert_eq!(default_camera(&mut app),Some(world));
            match app.world().get::<Camera>(root).unwrap().output_mode {
                bevy::camera::CameraOutputMode::Write {blend_state,clear_color}=>{
                    assert_eq!(blend_state,Some(bevy::render::render_resource::BlendState::ALPHA_BLENDING));
                    assert!(matches!(clear_color,ClearColorConfig::None));
                },
                _=>panic!("dialogue overlay must alpha-compose after world output"),
            }
            for (_,r) in rows(&mut app) {
                assert!(r.width()>200.0*density && r.height()>=48.0*density-0.01);
            }
            assert_eq!((app.world().resource::<TouchDialogue>().choice_scale,app.world().resource::<TouchDialogue>().subtitle_scale),(0.85,1.0));
        }
    }
}
#[test]
fn current_speaker_changes_reframe_and_menu_pause_free_camera_restore_prior_camera() {
    for cause in 0..7 {
        let (mut app,_,world)=scene(Side::Left,1.0);
        app.world_mut().resource_mut::<crate::dialogue_look::SpeakerFace>().0=Some((42,[-60.0,200.0,155.0]));
        app.update();
        let transform=app.world().get::<Transform>(world).unwrap();
        let face=crate::space::to_bevy([-60.0,200.0,155.0]);
        let view=transform.rotation.inverse()*(face-transform.translation);
        assert!(view.x<0.0 && view.z<0.0,"changed speaker remains on requested left side");
        match cause {
            0=>{app.insert_resource(State::new(crate::PlayState::Menu));},
            1=>{app.insert_resource(crate::pause::Paused(true));},
            2=>{app.world_mut().resource_mut::<crate::script::ScriptState>().rt.as_mut().unwrap().game.dialogue=None;},
            3=>{app.insert_resource(crate::player::ViewDriven(true));},
            4=>{app.world_mut().resource_mut::<TouchDialogue>().enabled=false;},
            5=>{app.world_mut().resource_mut::<Options>().side=Side::Standard;},
            _=>{app.world_mut().resource_mut::<crate::dialogue_look::SpeakerFace>().0=None;},
        }
        app.update();
        assert!(app.world().get::<Camera>(world).unwrap().viewport.is_none());
        assert_eq!(app.world().get::<Transform>(world).unwrap().rotation,Quat::IDENTITY);
        assert!(app.world().resource::<crate::dialogue_split::Frame>().ui_camera.is_none());
    }
}
#[test]
fn safe_area_orientation_and_layout_change_cancel_old_contact_then_fresh_release_works() {
    let (mut app,window,_)=scene(Side::Left,1.0);
    app.world_mut().resource_mut::<TouchDialogue>().confirm=false;
    settle(&mut app);
    let at=rows(&mut app)[0].1.center();
    send(&mut app,window,12,TouchPhase::Started,at);
    app.update();
    app.world_mut().resource_mut::<Options>().side=Side::Right;
    send(&mut app,window,12,TouchPhase::Ended,at);
    app.update();
    assert!(app.world().resource::<TouchFrame>().asked.pick.is_none());
    settle(&mut app);
    let at=rows(&mut app)[0].1.center();
    send(&mut app,window,13,TouchPhase::Started,at);
    app.update();
    send(&mut app,window,13,TouchPhase::Ended,at);
    app.update();
    assert_eq!(app.world().resource::<TouchFrame>().asked.pick,Some(0));
    assert!(SafeInsets::from_snapshot("1,1584,726,48,24,20,36",UVec2::new(1584,726),2.0).is_some());
    assert!(SafeInsets::from_snapshot("1,726,1584,48,24,20,36",UVec2::new(1584,726),2.0).is_none());
    assert!(SafeInsets::from_snapshot("1,1584,726,NaN,24,20,36",UVec2::new(1584,726),2.0).is_none());
}
#[test]
fn split_choice_persists_standard_and_both_sides_without_touching_text_preferences() {
    let path=std::env::temp_dir().join(format!("nvf-speaker-side-{}-{}.ini",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    for name in ["Standard","Speaker left","Speaker right"] {
        let mut app=App::new();
        install(&mut app);
        app.insert_resource(crate::settings::SettingsFile(Some(path.clone())));
        let index=app.world().resource::<crate::settings::Settings>().index(crate::dialogue_split::KEY).unwrap();
        // pick intentionally refuses an already-selected choice. Start elsewhere
        // on both platforms so every named choice exercises a real persisted edit.
        app.world_mut().resource_mut::<Options>().side=if name=="Speaker left" {Side::Right}else{Side::Left};
        assert!(crate::settings::pick(app.world_mut(),index,name));
        assert!(!crate::settings::pick(app.world_mut(),index,name),"already-selected choice remains a no-op");
        let mut loaded=App::new();
        install(&mut loaded);
        loaded.insert_resource(crate::settings::SettingsFile(Some(path.clone())));
        crate::settings::apply_stored(loaded.world_mut());
        assert_eq!(loaded.world().resource::<Options>().side,match name {"Speaker left"=>Side::Left,"Speaker right"=>Side::Right,_=>Side::Standard});
        assert_eq!(loaded.world().resource::<TouchDialogue>().choice_scale,TouchDialogue::default().choice_scale);
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn camera_owner_handoff_keeps_foreign_viewport_rotation_and_default_ui_marker() {
    let (mut app,_,world)=scene(Side::Left,1.0);
    let foreign=app.world_mut().spawn((Camera2d,bevy::ui::IsDefaultUiCamera,Camera {order:-5,..default()})).id();
    let viewport=bevy::camera::Viewport {physical_position:UVec2::new(10,20),physical_size:UVec2::new(500,250),..default()};
    let rotation=Quat::from_rotation_y(0.7);
    app.world_mut().get_mut::<Camera>(world).unwrap().viewport=Some(viewport.clone());
    app.world_mut().get_mut::<Transform>(world).unwrap().rotation=rotation;
    app.insert_resource(crate::player::ViewDriven(true));
    app.update();
    assert!(app.world().get::<bevy::ui::IsDefaultUiCamera>(world).is_none());
    assert!(app.world().get::<bevy::ui::IsDefaultUiCamera>(foreign).is_some());
    assert_eq!(default_camera(&mut app),Some(foreign));
    let actual=app.world().get::<Camera>(world).unwrap().viewport.as_ref().unwrap();
    assert_eq!((actual.physical_position,actual.physical_size),(viewport.physical_position,viewport.physical_size));
    assert_eq!(app.world().get::<Transform>(world).unwrap().rotation,rotation);
}

#[test]
fn ordinary_view_is_untouched_and_framing_follows_aspect_and_current_fov() {
    let (mut app,window,world)=scene(Side::Standard,1.0);
    assert!(app.world().get::<Camera>(world).unwrap().viewport.is_none());
    assert_eq!(app.world().get::<Transform>(world).unwrap().rotation,Quat::IDENTITY);
    for (width,height) in [(640.0,360.0),(1920.0,1080.0),(2400.0,1080.0),(800.0,1200.0)] {
        app.world_mut().get_mut::<Window>(window).unwrap().resolution.set(width,height);
        for fov in [0.15,0.65,1.1] {
            if let Projection::Perspective(p)=&mut *app.world_mut().get_mut::<Projection>(world).unwrap() {p.fov=fov;}
            for side in [Side::Left,Side::Right] {
                app.world_mut().resource_mut::<Options>().side=side;
                settle(&mut app);
                let t=app.world().get::<Transform>(world).unwrap();
                let view=t.rotation.inverse()*(crate::space::to_bevy([35.0,180.0,130.0])-t.translation);
                let screen_x=(0.5+0.5*view.x/(-view.z*(fov*0.5).tan()*(width/height)))*width;
                let panel=app.world().resource::<crate::dialogue_split::Frame>().panel.unwrap();
                assert!(if side==Side::Left {screen_x<panel.min.x}else{screen_x>panel.max.x});
                assert!(screen_x>0.0 && screen_x<width);
                assert!(app.world().get::<Camera>(world).unwrap().viewport.is_none());
            }
        }
    }
    app.world_mut().resource_mut::<Options>().side=Side::Standard;
    app.update();
    assert_eq!(app.world().get::<Transform>(world).unwrap().rotation,Quat::IDENTITY);
}
