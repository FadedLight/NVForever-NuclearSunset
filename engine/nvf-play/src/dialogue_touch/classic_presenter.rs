//! Supplied Classic hardware around live dialogue, never captured scene or text.
use super::*;
use super::classic::{Cache, Layout};
use crate::{interface::Face, touch_safe_area::SafeInsets};
use nvf_assets::GameData;

/// Prepared asset/font/layout input. The camera lease separately admits its actual
/// world target; this resource alone never means a frame was rendered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Prepared {
    pub window: Entity,
    pub size: Vec2,
    pub density: f32,
    pub ui_scale: f32,
    pub font:bevy::asset::AssetId<Image>,
    pub casing:bevy::asset::AssetId<Image>,
    pub origin: Vec2,
    pub canvas_size: Vec2,
    pub layout: Layout,
}
#[derive(Resource, Default)]
pub(crate) struct Surface(pub Option<Prepared>);
#[derive(Resource, Default)]
struct FontCache {faces:Option<crate::terminal::screen::Faces>,warned:bool,geometry_warned:bool}
#[derive(Component)]
struct ClassicRoot;

pub(super) fn install(app: &mut App) {
    classic::install(app);
    app.init_resource::<Surface>().init_resource::<FontCache>()
        .add_systems(Update, prepare.after(crate::PlaySet::Scripts)
            .before(crate::dialogue_split::Resolve).before(crate::PlaySet::Ui))
        .add_systems(Update, draw.in_set(crate::PlaySet::Ui)
            .after(crate::dialogue_menu::DrawMenu).before(super::draw));
}

const CAPS_SIZE:f32=12.0;
fn caps_width(face:&Face)->Option<f32> {
    if !face.font.size.is_finite() || face.font.size<=0.0 {return None;}
    let mut digit=0.0_f32;
    for c in b'0'..=b'9' {
        let advance=face.font.glyph(c).advance();
        if !advance.is_finite() || advance<=0.0 {return None;}
        digit=digit.max(advance);
    }
    let width=(digit*10.0+face.font.glyph(b'-').advance())*CAPS_SIZE/face.font.size+4.0;
    (width.is_finite() && width>0.0).then_some(width)
}

fn geometry_with_budget(size:Vec2,safe:SafeInsets,caps_width:f32)->Option<(Vec2,Layout)> {
    let safe=SafeInsets::from_array([safe.left,safe.top,safe.right,safe.bottom])?;
    let origin=Vec2::new(safe.left,safe.top);
    Some((origin,Layout::new(size-origin-Vec2::new(safe.right,safe.bottom),caps_width)?))
}

/// This is the one effective-presentation predicate used by the parent input,
/// native fallback and capture admission. Prepared assets alone are insufficient.
pub(super) fn ready_for(window:Entity,surface:Option<&Surface>,frame:&crate::dialogue_split::Frame)->bool {
    surface.and_then(|s|s.0.as_ref()).is_some_and(|p| {
        let world=Rect::from_corners(p.layout.world.min+p.origin,p.layout.world.max+p.origin);
        let min=(world.min*p.density).floor();let max=(world.max*p.density).ceil();
        p.window==window && frame.ui_camera.is_some() && frame.classic.is_some_and(|v|
            v.prepared==*p && v.window==window && v.world==world && v.window_size==p.size
                && v.density==p.density && v.ui_scale==p.ui_scale
                && v.physical_position==min.as_uvec2() && v.physical_size==(max-min).as_uvec2())
    })
}

type AssetInputs<'w>=(Option<Res<'w,GameData>>,Option<ResMut<'w,Assets<Image>>>,
    Option<Res<'w,bevy::image::CompressedImageFormatSupport>>);
#[allow(clippy::too_many_arguments)]
fn prepare(menu: Res<DialogueMenu>, settings: Res<TouchDialogue>, appearance: Res<appearance::Appearance>,
    safe: Res<SafeInsets>, gestures: Res<Gestures>, windows: Query<(Entity,&Window),With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>, (data,images,formats): AssetInputs<'_>,
    (state,paused,script,creator): Availability<'_>, mut cache: ResMut<Cache>,
    mut fonts: ResMut<FontCache>, mut surface: ResMut<Surface>) {
    let next=(|| {
        if appearance.theme!=appearance::Theme::ClassicGlass || !settings.enabled
            || !gestures.focused || !gestures.running
            || !available(state.as_deref(),paused.as_deref(),script.as_deref(),creator.as_deref()) {return None;}
        let (entity,window)=windows.single().ok()?;
        let density=window.scale_factor();
        let ui_scale=scale.as_ref().map_or(1.0,|s|s.0);
        if !window.focused || !density.is_finite() || density<=0.0
            || !ui_scale.is_finite() || ui_scale<=0.0 {return None;}
        menu.view.as_ref()?;
        let mut images=images?;
        cache.prepare(data.as_deref(),&mut images);
        let casing=cache.loaded()?.image_id();
        let data=data.as_deref()?;
        let faces=fonts.faces.get_or_insert_with(||crate::terminal::screen::load_faces(Some(data),&mut images,
            crate::interface::bc_supported(formats.as_deref())));
        // A default-font stand-in is not the supplied-art Classic presentation.
        if faces.text.image.is_none() {
            if !fonts.warned {warn!("Classic dialogue needs the installed Font 5 image; using the normal dialogue view");fonts.warned=true;}
            return None;
        }
        let size=Vec2::new(window.width(),window.height());
        let cap_budget=caps_width(&faces.text)?;
        let font=faces.text.image.as_ref()?.0.id();
        let Some((origin,layout))=geometry_with_budget(size,*safe,cap_budget) else {
            if !fonts.geometry_warned {warn!("Classic dialogue has no usable safe layout; using the normal dialogue view");fonts.geometry_warned=true;}
            return None;
        };
        Some(Prepared {window:entity,size,density,ui_scale,font,casing,origin,
            canvas_size:size-origin-Vec2::new(safe.right,safe.bottom),layout})
    })();
    if surface.0!=next {surface.0=next;}
}

#[derive(Clone,PartialEq)]
struct Key {surface:Prepared,camera:Entity,view:crate::dialogue_menu::MenuView,
    list:Option<ListId>,selected:usize,settings:TouchDialogue,armed:bool,
    spoken:Option<String>,speaking:bool,caps:Option<i32>,barter:Option<classic_state::BarterTarget>}
#[allow(clippy::too_many_arguments)]
fn draw(mut commands:Commands, prepared_surface:Res<Surface>, frame:Res<crate::dialogue_split::Frame>,
    menu:Res<DialogueMenu>, settings:Res<TouchDialogue>, gestures:Res<Gestures>, cache:Res<Cache>, fonts:Res<FontCache>,
    state:Res<classic_state::State>,
    roots:Query<Entity,With<ClassicRoot>>, mut shown:Local<Option<Key>>, mut reveal:ResMut<RevealRequest>) {
    let key=prepared_surface.0.as_ref().and_then(|prepared| {
        let surface=prepared;
        if !ready_for(surface.window,Some(&*prepared_surface),&frame) {return None;}
        Some(Key {surface:*surface,camera:frame.ui_camera?,view:menu.view.clone()?,
            list:menu.list.clone(),selected:menu.selected,settings:*settings,
            armed:gestures.armed==Some(menu.selected),spoken:state.spoken.clone(),speaking:state.speaking,
            caps:state.caps,barter:state.barter.clone()})
    });
    if *shown==key {return;}
    for root in &roots {commands.entity(root).despawn();}
    if let (Some(key),Some(art),Some(faces))=(key.as_ref(),cache.loaded(),fonts.faces.as_ref()) {
        spawn(&mut commands,key,art,&faces.text);
        reveal.0=true;
    }
    *shown=key;
}

fn node(rect:Rect,scale:f32)->Node {
    Node {position_type:PositionType::Absolute,left:px(rect.min.x/scale),top:px(rect.min.y/scale),
        width:px(rect.width()/scale),height:px(rect.height()/scale),..default()}
}
fn label(parent:&mut ChildSpawnerCommands,key:&Key,text:&str,size:f32,width:f32,face:&Face,color:Color) {
    device_canvas::label_on(parent,text,face,size,color,device_canvas::LabelViewport {
        width, ui_scale:key.surface.ui_scale, height:key.surface.size.y,
    });
}
fn spawn(commands:&mut Commands,key:&Key,art:&classic::LoadedArt,face:&Face) {
    let surface=&key.surface;let layout=&surface.layout;let scale=surface.ui_scale;
    // The root covers only the safe canvas. Hardware children are passive and
    // do not add touch targets over the upper live world opening.
    let mut root_node=node(Rect::from_corners(surface.origin,surface.origin+surface.canvas_size),scale);
    root_node.overflow=Overflow::clip();
    commands.spawn((ClassicRoot,PaletteRoot,Pickable::IGNORE,GlobalZIndex(2400),UiTargetCamera(key.camera),root_node))
        .with_children(|root| {
            for rect in layout.wells {
                root.spawn((Pickable::IGNORE,node(rect,scale),BackgroundColor(crate::terminal::screen::screen_color())));
            }
            // Real supplied housing is opaque, independent of the legacy frame slider.
            art.spawn(root,layout,scale,1.0);
            speech(root,key,face);
            responses(root,key,face);
            if let Some(caps)=key.caps {
                let text=caps.to_string();
                // Layout already reserves the actual font's complete i32 width.
                // Never shrink below the readable minimum or truncate a value.
                root.spawn((Pickable::IGNORE,node(layout.caps,scale))).with_children(|p|
                    label(p,key,&text,CAPS_SIZE,layout.caps.width(),face,crate::terminal::screen::bright()));
            }
            if let Some(target)=&key.barter {
                root.spawn((Button,native_barter(Some(target.list.clone())),node(layout.barter,scale),
                    Name::new("Classic barter button")));
            } else {
                root.spawn((Pickable::IGNORE,node(layout.red_button,scale),
                    BackgroundColor(Color::BLACK.with_alpha(0.65)),Name::new("Barter unavailable")));
            }

        });
}
fn speech(root:&mut ChildSpawnerCommands,key:&Key,face:&Face) {
    let rect=key.surface.layout.speech;let scale=key.surface.ui_scale;
    let mut n=node(rect,scale);n.overflow=Overflow::scroll_y();n.flex_direction=FlexDirection::Column;
    root.spawn((Viewport,ScrollPosition::default(),n)).with_children(|p| {
        let content=Node {width:percent(100.0),min_height:px(48.0/scale),flex_shrink:0.0,flex_direction:FlexDirection::Column,..default()};
        let mut child=p.spawn((Pickable::IGNORE,content));
        if key.speaking {child.insert((Button,native_skip(key.list.clone())));}
        child.with_children(|p| {
            if !key.view.speaker.trim().is_empty() {
                label(p,key,&key.view.speaker,18.0,rect.width(),face,crate::terminal::screen::bright());
            }
            if let Some(line)=&key.spoken {
                label(p,key,line,20.0*super::scale(key.settings.subtitle_scale),rect.width(),face,crate::terminal::screen::bright());
            }
        });
    });
}

fn responses(root:&mut ChildSpawnerCommands,key:&Key,face:&Face) {
    let rect=key.surface.layout.responses;let scale=key.surface.ui_scale;
    let mut n=node(rect,scale);n.overflow=Overflow::clip();n.flex_direction=FlexDirection::Column;
    root.spawn((Pickable::IGNORE,n)).with_children(|p| {
        if key.view.choices.is_empty() {
            if !key.speaking {return;}
            p.spawn((Button,native_skip(key.list.clone()),Node {width:percent(100.0),min_height:px(48.0/scale),
                padding:UiRect::all(px(6.0/scale)),flex_shrink:0.0,..default()})).with_children(|p|
                label(p,key,"Continue",18.0,rect.width()-12.0,face,crate::terminal::screen::bright()));
            return;
        }
        p.spawn((Viewport,ScrollPosition::default(),Node {width:percent(100.0),flex_grow:1.0,min_height:px(0.0),
            overflow:Overflow::scroll_y(),flex_direction:FlexDirection::Column,..default()})).with_children(|p| {
            for (index,row) in key.view.choices.iter().enumerate() {
                let selected=index==key.selected;
                let ink=if row.short {appearance::check_line()}
                    else if selected {Color::srgb(0.95,0.9,0.2)}
                    else if row.darkened {crate::terminal::screen::read_green()}
                    else {crate::terminal::screen::bright()};
                p.spawn((Button,native_choice(index,key.list.clone(),row.short),Node {width:percent(100.0),
                    min_height:px(48.0/scale),padding:UiRect::all(px(6.0/scale)),flex_shrink:0.0,..default()},
                    Outline::new(px(1.0/scale),px(-1.0/scale),if selected {ink}else{Color::NONE})))
                    .with_children(|p|label(p,key,&row.label,20.0*super::scale(key.settings.choice_scale),
                        rect.width()-12.0,face,ink));
            }
        });
        if key.settings.confirm {
            p.spawn((Button,native_confirm(key.list.clone()),Node {width:percent(100.0),min_height:px(48.0/scale),
                padding:UiRect::all(px(6.0/scale)),flex_shrink:0.0,..default()})).with_children(|p|
                label(p,key,if key.armed {"Confirm selection"}else{"Select a choice first"},18.0,
                    rect.width()-12.0,face,crate::terminal::screen::bright()));
        }
    });
}


#[cfg(test)]
pub(crate) use tests::with_barter_surface;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dialogue_touch::compact_tests::{layout_scene,rows,send,settle};
    use crate::dialogue_split::{Options,Side};
    use bevy::camera::{ComputedCameraValues,RenderTargetInfo};

    // CPU scheduling/layout fixture only. This writes authored target metadata;
    // it does not run CameraUpdate or establish GPU/window-provider readiness.
    fn publish_targets(windows:Query<&Window,With<PrimaryWindow>>,mut cameras:Query<&mut Camera>) {
        let window=windows.single().unwrap();
        for mut camera in &mut cameras {
            camera.computed=ComputedCameraValues {target_info:Some(RenderTargetInfo {
                physical_size:UVec2::new(window.physical_width(),window.physical_height()),
                scale_factor:window.scale_factor()}),..default()};
        }
    }
    fn scene(width:f32,density:f32,ui_scale:f32,font:bool)->(App,Entity,Entity,classic::tests::Files) {
        let (mut app,window)=layout_scene(true,Vec2::new(width,360.0),density);
        let (files,data)=classic::tests::files(font);
        app.insert_resource(data).insert_resource(Assets::<Image>::default()).insert_resource(UiScale(ui_scale));
        app.configure_sets(Update,(crate::PlaySet::Input,crate::PlaySet::Physics,crate::PlaySet::Activation,
            crate::PlaySet::Scripts,crate::PlaySet::Ui,crate::PlaySet::Sync).chain());
        let mut script=crate::script::ScriptState::over_no_plugins();
        script.rt.as_mut().unwrap().game.dialogue=Some(fnv_game::dialogue::Conversation {
            actor:fnv_sim::FormId(42),through:None,line:None,choices:vec![],topic_list:false,
            lines:1,then:None,ending:false,
        });
        app.insert_resource(script).insert_resource(Options {side:Side::Left})
            .insert_resource(SafeInsets {left:12.0,top:6.0,right:10.0,bottom:8.0})
            .add_systems(Update,publish_targets.before(crate::dialogue_split::Resolve));
        app.world_mut().resource_mut::<appearance::Appearance>().theme=appearance::Theme::ClassicGlass;
        app.world_mut().resource_mut::<TouchDialogue>().confirm=false;
        let camera=app.world_mut().spawn((crate::PlayerCamera,Camera3d::default(),
            Camera {order:1,..default()},Transform::from_xyz(0.0,112.0,0.0))).id();
        settle(&mut app);
        (app,window,camera,files)
    }
    pub(crate) fn with_barter_surface(test: impl FnOnce(App, Entity)) {
        let (app, window, _, _files) = scene(800., 1., 1., true);
        test(app, window);
    }
    fn lease(app:&App)->crate::dialogue_split::ClassicViewport {
        app.world().resource::<crate::dialogue_split::Frame>().classic.expect("real prepared Classic camera lease")
    }
    fn release(app:&mut App,window:Entity,id:u64,point:Vec2) {
        send(app,window,id,TouchPhase::Started,point);app.update();
        send(app,window,id,TouchPhase::Ended,point);app.update();
    }
    #[test]
    fn real_vfs_font_and_authored_camera_metadata_publish_safe_classic_rows_at_each_density_and_scale() {
        for (width,density,scale) in [(640.,1.,0.75),(640.,2.,1.5),(800.,1.,1.5),(800.,2.,1.)] {
            let (mut app,window,camera,_files)=scene(width,density,scale,true);
            assert!(app.world().resource::<Cache>().loaded().is_some());
            assert!(app.world().resource::<FontCache>().faces.as_ref().unwrap().text.image.is_some());
            let frame=*app.world().resource::<crate::dialogue_split::Frame>();
            assert!(ready_for(window,Some(app.world().resource::<Surface>()),&frame));
            let request=lease(&app);
            let mut glyphs=app.world_mut().query::<(&ImageNode,&ComputedNode)>();
            assert!(glyphs.iter(app.world()).any(|(image,node)|image.image.id()==request.prepared.font && node.size().min_element()>0.0),"actual loaded Font5 glyph node");
            let actual=app.world().get::<Camera>(camera).unwrap().viewport.as_ref().unwrap();
            assert_eq!((actual.physical_position,actual.physical_size),(request.physical_position,request.physical_size));
            assert_eq!(app.world().get::<Transform>(camera).unwrap().rotation,Quat::IDENTITY);
            let overlay=frame.ui_camera.unwrap();assert_ne!(overlay,camera);
            assert!(app.world().get::<Camera>(overlay).unwrap().viewport.is_none());
            for (_,row) in rows(&mut app) {
                assert!(row.height()>=48.*density-0.01,"real minimum target: width={width} density={density} UiScale={scale} {row:?}");
            }
            let npc=request.prepared.layout.speech.center()+request.prepared.origin;
            release(&mut app,window,70,npc);
            assert!(app.world().resource::<TouchFrame>().asked.pick.is_none());
            assert!(!app.world().resource::<TouchFrame>().asked.skip,"retained NPC text is not a Skip action while choosing");
            let at=rows(&mut app)[0].1.center()/density;release(&mut app,window,71,at);
            assert_eq!(app.world().resource::<TouchFrame>().asked.pick,Some(0));
            let mut passive=app.world_mut().query::<(&ImageNode,&Pickable)>();
            assert!(passive.iter(app.world()).filter(|(image,_)|image.rect.is_some()).all(|(_,pick)|*pick==Pickable::IGNORE));
        }
        let (app,window,_,_files)=scene(640.,1.,1.,false);
        assert!(app.world().resource::<Cache>().loaded().is_some(),"art alone is insufficient");
        assert!(!ready_for(window,Some(app.world().resource::<Surface>()),app.world().resource::<crate::dialogue_split::Frame>()));
        assert!(app.world().resource::<Surface>().0.is_none(),"missing real Font5 never publishes Classic ready");
    }
    #[test]
    fn classic_lease_restores_owned_viewport_on_exit_and_preserves_foreign_camera_ownership() {
        for cause in 0..7 {
            let (mut app,window,camera,_files)=scene(640.,1.,1.,true);let before=lease(&app);
            match cause {
                0=>{app.insert_resource(State::new(crate::PlayState::Menu));},
                1=>{app.insert_resource(crate::pause::Paused(true));},
                2=>{app.world_mut().get_mut::<Window>(window).unwrap().focused=false;},
                3=>{app.world_mut().resource_mut::<TouchDialogue>().enabled=false;},
                4=>{app.world_mut().resource_mut::<appearance::Appearance>().theme=appearance::Theme::Minimal;},
                5=>{app.world_mut().resource_mut::<crate::script::ScriptState>().rt.as_mut().unwrap().game.dialogue=None;},
                _=>{app.world_mut().get_mut::<Camera>(camera).unwrap().is_active=false;},
            }
            app.update();
            assert!(app.world().get::<Camera>(camera).unwrap().viewport.is_none(),"owned restore cause={cause} previous={before:?}");
            assert!(app.world().resource::<crate::dialogue_split::Frame>().classic.is_none());
            assert_eq!(app.world().get::<Transform>(camera).unwrap().rotation,Quat::IDENTITY);
        }
        for reset in [false,true] {
            let (mut app,_,camera,_files)=scene(640.,1.,1.,true);
            let foreign=bevy::camera::Viewport {physical_position:UVec2::new(9,11),physical_size:UVec2::new(320,200),..default()};
            let rotation=Quat::from_rotation_y(0.4);
            app.world_mut().get_mut::<Camera>(camera).unwrap().viewport=if reset {None}else{Some(foreign.clone())};
            app.world_mut().get_mut::<Transform>(camera).unwrap().rotation=rotation;
            let default=app.world_mut().spawn((Camera2d,bevy::ui::IsDefaultUiCamera,Camera {order:-5,..default()})).id();
            settle(&mut app);
            let actual=app.world().get::<Camera>(camera).unwrap().viewport.as_ref();
            if reset {assert!(actual.is_none());}else {
                assert_eq!(actual.map(|v|(v.physical_position,v.physical_size)),Some((foreign.physical_position,foreign.physical_size)));
            }
            assert!(app.world().resource::<crate::dialogue_split::Frame>().classic.is_none());
            assert!(app.world().get::<bevy::ui::IsDefaultUiCamera>(default).is_some());
            assert_eq!(app.world().get::<Transform>(camera).unwrap().rotation,rotation);
        }
    }
    #[test]
    fn safe_layout_density_and_ui_scale_changes_retire_old_release_then_current_rows_recover() {
        for cause in 0..3 {
            let (mut app,window,_,_files)=scene(800.,1.,1.,true);let original=lease(&app);
            let at=rows(&mut app)[0].1.center();send(&mut app,window,81,TouchPhase::Started,at);app.update();
            match cause {
                0=>app.world_mut().resource_mut::<SafeInsets>().left+=12.,
                1=>app.world_mut().resource_mut::<UiScale>().0=1.5,
                _=>{
                    let mut w=app.world_mut().get_mut::<Window>(window).unwrap();
                    w.resolution.set_scale_factor_override(Some(2.0));w.resolution.set(800.0,360.0);
                },
            }
            // One ordinary frame publishes the new layout/camera metadata. The
            // old contact cannot authorize a response under the new surface.
            app.update();send(&mut app,window,81,TouchPhase::Ended,at);app.update();
            assert!(app.world().resource::<TouchFrame>().asked.pick.is_none());
            settle(&mut app);assert_ne!(lease(&app),original);
            let density=app.world().get::<Window>(window).unwrap().scale_factor();
            let at=rows(&mut app)[0].1.center()/density;release(&mut app,window,82,at);
            assert_eq!(app.world().resource::<TouchFrame>().asked.pick,Some(0));
        }
    }
    #[test]
    fn complete_zero_four_digit_and_extreme_caps_keep_readable_native_glyphs_inside_the_reflowed_bezel() {
        let (_files,data)=classic::tests::files(true);let mut images=Assets::<Image>::default();
        let faces=crate::terminal::screen::load_faces(Some(&data),&mut images,false);
        assert!(faces.text.image.is_some(),"real VFS Font5 path, not a default-font substitute");
        let budget=caps_width(&faces.text).unwrap();
        let font=native::scaled_face(&faces.text,CAPS_SIZE/faces.text.font.size);
        for width in [640.,800.] {
            let layout=Layout::new(Vec2::new(width,360.),budget).unwrap();
            assert!(layout.caps.height()>=font.font.size*1.2);
            for value in [0,9999,i32::MAX,i32::MIN] {
                let text=value.to_string();let lines=font.font.lines(&text,Some(layout.caps.width()));
                assert_eq!(lines.len(),1,"caps={value} width={width}");
                assert_eq!(lines[0].text(),text,"complete exact value, never a compact/truncated amount");
                assert!(lines[0].width<=layout.caps.width());
                let baseline=crate::interface::line_base(&font);
                for placed in &lines[0].glyphs {
                    let g=font.font.glyph(placed.code);
                    assert!(placed.x+g.before>=0.0 && placed.x+g.before+g.width<=layout.caps.width());
                    assert!(baseline-g.ascent>=-0.01 && baseline-g.ascent+g.height<=layout.caps.height()+0.01);
                }
            }
            let overlap=layout.caps.intersect(layout.barter);
            assert!(overlap.width()<=0.0 || overlap.height()<=0.0,"cash and live Barter never share a target");
        }
    }

    #[derive(Resource)]
    struct CameraHandoff {camera:Entity,other_window:Entity,mode:u8,fired:bool,proposal_uncommitted:bool}
    fn handoff_between_resolve_and_admit(mut handoff:ResMut<CameraHandoff>,
        frame:Res<crate::dialogue_split::Frame>,surface:Res<Surface>,
        mut cameras:Query<(&mut Camera,&mut bevy::camera::RenderTarget)>) {
        if handoff.fired {return;}
        handoff.fired=true;
        handoff.proposal_uncommitted=frame.classic.is_none()
            && surface.0.as_ref().is_some_and(|p|!ready_for(p.window,Some(&surface),&frame));
        let (mut camera,mut target)=cameras.get_mut(handoff.camera).unwrap();
        match handoff.mode {
            0=>{},
            1=>camera.viewport=Some(bevy::camera::Viewport {physical_position:UVec2::new(17,19),
                physical_size:UVec2::new(320,170),..default()}),
            2=>*target=bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(handoff.other_window)),
            _=>camera.is_active=false,
        }
    }
    #[test]
    fn actual_camera_handoff_is_admitted_or_refused_before_either_painter_selects_its_frame() {
        for mode in 0..4 {
            let (mut app,window,camera,_files)=scene(800.,1.,1.,true);
            app.init_resource::<crate::pad::PadGlyphs>();
            let original=lease(&app);
            let overlay=app.world().resource::<crate::dialogue_split::Frame>().ui_camera.unwrap();
            let at=rows(&mut app)[0].1.center();
            let other_window=app.world_mut().spawn(Window::default()).id();
            app.insert_resource(CameraHandoff {camera,other_window,mode,fired:false,proposal_uncommitted:false})
                .add_systems(Update,handoff_between_resolve_and_admit
                    .after(crate::dialogue_split::Resolve).before(crate::dialogue_split::Admit));
            send(&mut app,window,91,TouchPhase::Started,at);app.update();
            assert!(app.world().resource::<CameraHandoff>().proposal_uncommitted,
                "Resolve cannot publish proposed Classic as painter-ready: mode={mode}");
            let frame=*app.world().resource::<crate::dialogue_split::Frame>();
            let ready=ready_for(window,Some(app.world().resource::<Surface>()),&frame);
            let classic_roots=app.world_mut().query_filtered::<Entity,With<ClassicRoot>>().iter(app.world()).count();
            let native_roots:Vec<_>=app.world_mut().query_filtered::<(Entity,&UiTargetCamera),
                With<crate::dialogue_menu::TouchNativeRoot>>().iter(app.world()).map(|(e,c)|(e,c.0)).collect();
            if mode==0 {
                assert!(ready && frame.classic==Some(original));
                assert_eq!(classic_roots,1);assert!(native_roots.is_empty());
                let actual=app.world().get::<Camera>(camera).unwrap().viewport.as_ref().unwrap();
                assert_eq!((actual.physical_position,actual.physical_size),(original.physical_position,original.physical_size));
                continue;
            }
            assert!(!ready && frame.classic.is_none());assert_eq!(classic_roots,0);
            assert_eq!(native_roots.len(),1,"real Native fallback must be selected in the same frame, mode={mode}");
            let fallback=native_roots[0].1;assert_ne!(fallback,camera);
            assert_eq!(Some(fallback),frame.ui_camera);assert_eq!(fallback,overlay,"one shared full-window UI facility");
            let fallback_camera=app.world().get::<Camera>(fallback).unwrap();
            assert!(fallback_camera.is_active && fallback_camera.viewport.is_none());
            assert_eq!(app.world().get::<bevy::camera::RenderTarget>(fallback).unwrap().normalize(Some(window)),
                bevy::camera::RenderTarget::default().normalize(Some(window)));
            match mode {
                1=>{
                    let v=app.world().get::<Camera>(camera).unwrap().viewport.as_ref().unwrap();
                    assert_eq!((v.physical_position,v.physical_size),(UVec2::new(17,19),UVec2::new(320,170)));
                },
                2=>assert_eq!(app.world().get::<bevy::camera::RenderTarget>(camera).unwrap().normalize(Some(window)),
                    bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(other_window)).normalize(Some(window))),
                _=>assert!(!app.world().get::<Camera>(camera).unwrap().is_active),
            }
            // The contact began on the old Classic presentation. Fallback cannot
            // reinterpret its terminal release as a new Normal response.
            send(&mut app,window,91,TouchPhase::Ended,at);app.update();
            assert!(app.world().resource::<TouchFrame>().asked.pick.is_none());
            app.world_mut().get_mut::<Camera>(camera).unwrap().viewport=None;
            app.world_mut().get_mut::<Camera>(camera).unwrap().is_active=true;
            *app.world_mut().get_mut::<bevy::camera::RenderTarget>(camera).unwrap()=bevy::camera::RenderTarget::default();
            app.update();
            assert!(app.world().resource::<crate::dialogue_split::Frame>().classic.is_none(),
                "a foreign takeover is not immediately reacquired while the same Classic view stays open");
            app.world_mut().resource_mut::<appearance::Appearance>().theme=appearance::Theme::Minimal;app.update();
            app.world_mut().resource_mut::<appearance::Appearance>().theme=appearance::Theme::ClassicGlass;settle(&mut app);
            assert!(ready_for(window,Some(app.world().resource::<Surface>()),app.world().resource::<crate::dialogue_split::Frame>()));
            let fresh=rows(&mut app)[0].1.center();release(&mut app,window,92,fresh);
            assert_eq!(app.world().resource::<TouchFrame>().asked.pick,Some(0));
        }
    }

    #[test]
    fn foreign_camera_change_before_scripts_withdraws_old_classic_before_terminal_touch_input() {
        for target_change in [false,true] {
            let (mut app,window,camera,_files)=scene(800.,1.,1.,true);
            app.init_resource::<crate::pad::PadGlyphs>();
            let at=rows(&mut app)[0].1.center();
            send(&mut app,window,101,TouchPhase::Started,at);app.update();
            let other_window=app.world_mut().spawn(Window::default()).id();
            if target_change {
                *app.world_mut().get_mut::<bevy::camera::RenderTarget>(camera).unwrap()=
                    bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(other_window));
            } else {
                app.world_mut().get_mut::<Camera>(camera).unwrap().viewport=Some(bevy::camera::Viewport {
                    physical_position:UVec2::new(21,23),physical_size:UVec2::new(300,150),..default()});
            }
            send(&mut app,window,101,TouchPhase::Ended,at);app.update();
            let asked=&app.world().resource::<TouchFrame>().asked;
            assert!(asked.pick.is_none() && !asked.skip,"the old applied frame cannot authorize release before Resolve: target={target_change}");
            assert!(app.world().resource::<crate::dialogue_split::Frame>().classic.is_none());
            let native=app.world_mut().query_filtered::<&UiTargetCamera,With<crate::dialogue_menu::TouchNativeRoot>>()
                .single(app.world()).unwrap().0;
            assert_eq!(Some(native),app.world().resource::<crate::dialogue_split::Frame>().ui_camera);
            if target_change {
                assert_eq!(app.world().get::<bevy::camera::RenderTarget>(camera).unwrap().normalize(Some(window)),
                    bevy::camera::RenderTarget::Window(bevy::window::WindowRef::Entity(other_window)).normalize(Some(window)));
            } else {
                let viewport=app.world().get::<Camera>(camera).unwrap().viewport.as_ref().unwrap();
                assert_eq!((viewport.physical_position,viewport.physical_size),(UVec2::new(21,23),UVec2::new(300,150)));
            }
        }
    }

}
