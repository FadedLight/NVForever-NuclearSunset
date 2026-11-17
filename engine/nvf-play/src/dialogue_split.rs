//! Full-world dialogue framing: move the speaker beside choices without cropping the scene.
use bevy::{camera::{visibility::RenderLayers, CameraOutputMode}, prelude::*, render::render_resource::BlendState, ui::{DefaultUiCamera,IsDefaultUiCamera}, window::PrimaryWindow};
use crate::{dialogue_menu::DialogueMenu, dialogue_touch::TouchDialogue, PlayerCamera};
use crate::touch_safe_area::SafeInsets;

pub const KEY: &str = "dialogue-speaker-side";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Side { #[default] Standard, Left, Right }
impl Side {
    fn name(self) -> &'static str { match self { Self::Standard => "Standard", Self::Left => "Speaker left", Self::Right => "Speaker right" } }
    fn named(name: &str) -> Option<Self> { [Self::Standard, Self::Left, Self::Right].into_iter().find(|s| s.name() == name) }
}
#[cfg_attr(test, derive(Debug))]
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct Options { pub side: Side }
impl Default for Options {
    fn default() -> Self { Self { side: if cfg!(target_os="android") { Side::Left } else { Side::Standard } } }
}
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Frame {
    pub panel: Option<Rect>,
    speaker: Option<Rect>,
    pub ui_camera: Option<Entity>,
}
#[derive(Resource, Default)]
struct Target(Option<Vec3>);
#[derive(Component)]
struct OverlayCamera;
#[derive(Component)]
struct OwnedDefault;
#[derive(Resource,Default)]
struct DefaultPin(Option<Entity>);
#[derive(Resource, Default)]
struct Lease(Option<Owned>);
struct Owned {
    entity: Entity,
    base_rotation: Quat,
    written_rotation: Quat,
}

fn chosen(w: &World) -> String { w.resource::<Options>().side.name().into() }
fn choose(w: &mut World, name: &str) -> bool {
    let Some(side) = Side::named(name) else { return false };
    w.resource_mut::<Options>().side = side;
    true
}
pub(crate) fn install(app: &mut App) {
    app.init_resource::<Options>().init_resource::<SafeInsets>().init_resource::<Frame>()
        .init_resource::<Target>().init_resource::<Lease>().init_resource::<DefaultPin>()
        .add_systems(Update, resolve.after(crate::PlaySet::Scripts).before(crate::PlaySet::Ui))
        .add_systems(Update, apply.in_set(crate::PlaySet::Sync).after(crate::control::place_camera).after(crate::dialogue_zoom::apply));
    crate::settings::add_choice_list(app, crate::settings::Setting {
        key:KEY, group:crate::pause::GROUP_MENUS, label:"Dialogue speaker side",
        note:"Frame the speaker beside choices over the full world. Choose the speaker side, or Standard for centered dialogue. Requires touch dialogue.",
        switches:"", get:|_|false,set:|_,_|{},slider:None,
    },crate::settings::ChoiceList { names:|_|[Side::Standard,Side::Left,Side::Right].map(|s|s.name().into()).to_vec(),
        chosen,pick:choose,save:chosen,load:choose,ask_keep:false });
}

fn rectangles(size: Vec2, safe: SafeInsets, side: Side) -> Option<(Rect,Rect)> {
    if side == Side::Standard || !size.is_finite() || size.x < 480.0 || size.y < 240.0 { return None }
    let safe = SafeInsets::from_array([safe.left,safe.top,safe.right,safe.bottom])?;
    let inner = Rect::from_corners(Vec2::new(safe.left+8.0,safe.top+8.0),size-Vec2::new(safe.right+8.0,safe.bottom+8.0));
    if inner.width() < 464.0 || inner.height() < 160.0 { return None }
    let mid = (inner.min.x+inner.max.x)*0.5;
    let left = Rect::from_corners(inner.min,Vec2::new(mid-4.0,inner.max.y));
    let right = Rect::from_corners(Vec2::new(mid+4.0,inner.min.y),inner.max);
    Some(if side==Side::Left {(left,right)}else{(right,left)})
}

#[allow(clippy::too_many_arguments)]
type SplitResources<'w> = (Res<'w,Options>, Res<'w,SafeInsets>, Res<'w,TouchDialogue>);
type SceneState<'w> = (Option<Res<'w,State<crate::PlayState>>>, Option<Res<'w,crate::pause::Paused>>, Option<Res<'w,crate::free_cam::FreeCam>>, Option<Res<'w,crate::control::PlayerControl>>, Option<Res<'w,crate::player::ViewDriven>>);
type SpeakerSources<'w> = (Option<Res<'w,crate::script::ScriptState>>, Option<Res<'w,crate::dialogue_look::SpeakerFace>>, Option<Res<'w,crate::phys::PhysState>>, Option<Res<'w,crate::sim::SimState>>);
type ResolveQueries<'w,'s> = (
    Query<'w,'s,&'static Window,With<PrimaryWindow>>,
    Query<'w,'s,&'static Camera,With<PlayerCamera>>,
    DefaultUiCamera<'w,'s>,
    Query<'w,'s,Entity,(With<Camera>,With<IsDefaultUiCamera>)>,
    Query<'w,'s,Entity,With<OwnedDefault>>,
    Query<'w,'s,Entity,With<OverlayCamera>>,
);
type ResolveOutput<'w> = (ResMut<'w,DefaultPin>,ResMut<'w,Frame>,ResMut<'w,Target>);
fn resolve(
    mut commands: Commands, (options,safe,touch): SplitResources<'_>,
    (state,paused,free,control,driven): SceneState<'_>, menu: Res<DialogueMenu>,
    (script,face,phys,sim): SpeakerSources<'_>,
    (windows,world_cameras,default_ui,default_markers,own_markers,overlays): ResolveQueries<'_,'_>,
    (mut pin,mut frame,mut target): ResolveOutput<'_>,
) {
    let mut next = Frame::default();
    target.0 = None;
    let foreign_change=pin.0.is_some_and(|p|!default_markers.contains(p) || default_markers.iter().any(|e|e!=p));
    let allowed = !foreign_change && touch.enabled && state.as_ref().is_some_and(|s|*s.get()==crate::PlayState::Dialogue)
        && menu.view.is_some() && !paused.is_some_and(|p|p.0) && !free.is_some_and(|f|f.on)
        && !driven.is_some_and(|d|d.0) && world_cameras.single().is_ok_and(|c|c.is_active && c.viewport.is_none())
        && !control.is_some_and(|c|c.cinematic || c.portrait.is_some());
    if allowed {
        if let (Ok(window),Some(game)) = (windows.single(),script.as_ref().and_then(|s|s.rt.as_ref()).map(|r|&r.game)) {
            if !crate::dialogue::held(game) {
                if let Some(conv) = game.dialogue.as_ref() {
                    let actor = conv.through.unwrap_or(conv.actor);
                    let volume = phys.as_ref().and_then(|p|p.world.actor_volume(actor.0)).map(|v|(v.feet,v.height));
                    let reference = sim.as_ref().and_then(|s|s.world.peek(actor).map(|r|r.pos).or_else(||s.refs.get(actor).map(|r|r.pos)));
                    let point = if conv.through.is_some() { reference } else {
                        crate::dialogue_look::face_point(actor.0,face.as_ref().and_then(|f|f.0),volume,reference).map(|p|p.0)
                    };
                    if let (Some(point),Some((world,panel))) = (point,rectangles(Vec2::new(window.width(),window.height()),*safe,options.side)) {
                        let point = crate::space::to_bevy(point);
                        if point.is_finite() {
                            if pin.0.is_none() && default_markers.is_empty() {
                                if let Some(entity)=default_ui.get() {
                                    commands.entity(entity).insert((IsDefaultUiCamera,OwnedDefault));
                                    pin.0=Some(entity);
                                }
                            }
                            let camera = overlays.single().ok().unwrap_or_else(||commands.spawn((
                                OverlayCamera,Name::new("Touch dialogue overlay camera"),Camera2d,
                                Camera {order:world_cameras.single().map_or(1,|c|c.order.saturating_add(1)),is_active:false,
                                    clear_color:ClearColorConfig::Custom(Color::NONE),
                                    output_mode:CameraOutputMode::Write {blend_state:Some(BlendState::ALPHA_BLENDING),clear_color:ClearColorConfig::None},..default()},
                                RenderLayers::none(),
                            )).id());
                            next=Frame { panel:Some(panel),speaker:Some(world),ui_camera:Some(camera) };
                            target.0=Some(point);
                        }
                    }
                }
            }
        }
    }
    if *frame!=next { *frame=next; }
    if next.ui_camera.is_none() {
        for entity in &overlays { commands.entity(entity).despawn(); }
        if let Some(entity)=pin.0.take().filter(|e|own_markers.contains(*e)) {
            commands.entity(entity).remove::<(IsDefaultUiCamera,OwnedDefault)>();
        }
    }
}

fn restore(owned: Owned, cameras: &mut Query<(Entity,&Camera,&Projection,&mut Transform),Without<OverlayCamera>>) {
    if let Ok((_,_,_,mut transform))=cameras.get_mut(owned.entity) {
        if transform.rotation==owned.written_rotation { transform.rotation=owned.base_rotation; }
    }
}

/// Aim the real camera so the speaker projects into the free half of the full view.
/// The world render target, viewport, projection and camera position remain owned by
/// their ordinary consumers; a foreign viewport suspends this framing.
fn framed_rotation(position: Vec3, point: Vec3, fov: f32, aspect: f32, screen_x: f32) -> Option<Quat> {
    let direction=point-position;
    if !direction.is_finite() || direction.length_squared()<0.0001
        || !fov.is_finite() || !(0.0..std::f32::consts::PI).contains(&fov)
        || !aspect.is_finite() || aspect<=0.0 || !screen_x.is_finite() { return None; }
    let center=Transform::from_translation(position).looking_at(point,Vec3::Y).rotation;
    let yaw=((2.0*screen_x-1.0)*(fov*0.5).tan()*aspect).atan();
    let rotation=center*Quat::from_rotation_y(yaw);
    rotation.is_finite().then_some(rotation)
}
type OverlayCameras<'w,'s> = Query<'w,'s,(Entity,&'static mut Camera),(With<OverlayCamera>,Without<PlayerCamera>)>;
fn apply(frame: Res<Frame>, target: Res<Target>, windows: Query<&Window,With<PrimaryWindow>>,
    players: Query<Entity,With<PlayerCamera>>,
    mut cameras: Query<(Entity,&Camera,&Projection,&mut Transform),Without<OverlayCamera>>,
    mut overlays: OverlayCameras<'_,'_>,mut lease: ResMut<Lease>) {
    for (entity,mut camera) in &mut overlays { camera.is_active=frame.ui_camera==Some(entity); }
    let active = frame.speaker.zip(target.0).zip(windows.single().ok());
    let Some(((rect,point),window))=active else {
        if let Some(owned)=lease.0.take() { restore(owned,&mut cameras); }
        return;
    };
    let Ok(entity)=players.single() else {
        if let Some(owned)=lease.0.take() {restore(owned,&mut cameras);}
        return;
    };
    if lease.0.as_ref().is_some_and(|l|l.entity!=entity) {
        if let Some(owned)=lease.0.take() {restore(owned,&mut cameras);}
    }
    let Ok((_,camera,projection,mut transform))=cameras.get_mut(entity) else {return};
    let Projection::Perspective(projection)=projection else {
        if let Some(owned)=lease.0.take() {restore(owned,&mut cameras);}
        return;
    };
    if camera.viewport.is_some() {
        if let Some(owned)=lease.0.take() {restore(owned,&mut cameras);}
        return;
    }
    let aspect=window.width()/window.height();
    let Some(rotation)=framed_rotation(transform.translation,point,projection.fov,aspect,rect.center().x/window.width()) else {
        if let Some(owned)=lease.0.take() {restore(owned,&mut cameras);}
        return;
    };
    let owned=lease.0.get_or_insert_with(||Owned {entity,base_rotation:transform.rotation,written_rotation:transform.rotation});
    if transform.rotation!=owned.written_rotation { owned.base_rotation=transform.rotation; }
    transform.rotation=rotation;
    owned.written_rotation=rotation;
}
