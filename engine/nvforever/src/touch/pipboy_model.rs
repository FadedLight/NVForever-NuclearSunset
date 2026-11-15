//! Touch targets follow the real posed device; they never synthesize mouse input.
use bevy::{prelude::*, input::touch::{TouchInput, TouchPhase}, ecs::system::SystemParam};
use nvf_play::pipboy::{Page, Phase, Frame, PipBoy, PipBoyInput, PipBoyScreen};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ButtonTarget {
    pub page: Page,
    pub bounds: Rect,
}
#[derive(Resource, Default, Clone, PartialEq)]
pub(crate) struct Geometry {
    pub buttons: Vec<ButtonTarget>,
    pub knob: Option<Rect>,
    pub screen: Option<Rect>,
    pub screen_triangles: Vec<[Vec2; 3]>,
    pub size: Vec2,
    pub image: Option<bevy::asset::AssetId<Image>>,
    pub page: Option<Page>,
    pub tab: usize,
    pub entities: Vec<Entity>,
    pub camera: Option<Entity>,
}
#[derive(Clone, PartialEq)]
struct Stamp { size: Vec2, image: Option<bevy::asset::AssetId<Image>>, page: Option<Page>, tab: usize, entities: Vec<Entity>, camera: Option<Entity> }
impl Geometry {
    fn stamp(&self) -> Stamp { Stamp { size: self.size, image: self.image, page: self.page, tab: self.tab, entities: self.entities.clone(), camera: self.camera } }
    fn on_screen(&self, point: Vec2) -> bool {
        self.screen_triangles.iter().any(|[a,b,c]| {
            let (u,v,p) = (*b-*a,*c-*a,point-*a); let determinant = u.perp_dot(v);
            if !determinant.is_finite() || determinant.abs() < 1e-5 { return false; }
            let x=p.perp_dot(v)/determinant; let y=u.perp_dot(p)/determinant;
            x>=0.0 && y>=0.0 && x+y<=1.0
        })
    }
    fn target(&self, point: Vec2) -> Option<ButtonTarget> {
        if !point.is_finite() || self.on_screen(point) { return None; }
        self.buttons.iter().copied().filter(|b| b.bounds.contains(point))
            .min_by(|a, b| a.bounds.center().distance_squared(point).total_cmp(&b.bounds.center().distance_squared(point)))
    }
    pub(crate) fn projected_bounds(camera: &Camera, at: &GlobalTransform, points: &[Vec3], size: Vec2) -> Option<Rect> {
        if !size.is_finite() || size.min_element() < 48.0 || !points.iter().all(|p| p.is_finite()) { return None; }
        let projected: Option<Vec<_>> = points.iter().map(|p| camera.world_to_viewport(at, *p).ok()).collect();
        let points = projected.filter(|p| !p.is_empty())?;
        if !points.iter().all(|p| p.is_finite()) { return None; }
        let low = points.iter().copied().fold(Vec2::INFINITY, Vec2::min);
        let high = points.iter().copied().fold(Vec2::NEG_INFINITY, Vec2::max);
        if !(high-low).is_finite() { return None; }
        let visible = Rect::from_corners(low, high).intersect(Rect::from_corners(Vec2::ZERO, size));
        if visible.width() <= 0.0 || visible.height() <= 0.0 { return None; }
        let extent = visible.size().max(Vec2::splat(48.0));
        if !extent.is_finite() || size.min_element() < 48.0 { return None; }
        let extent = extent.min(size);
        let center = visible.center().clamp(extent * 0.5, size - extent * 0.5);
        Some(Rect::from_center_size(center, extent))
    }
}
#[derive(Clone, Copy)]
enum Kind { Button(ButtonTarget), Knob { previous: Vec2, remainder: f32 } }
#[derive(Clone)]
struct Contact { id: u64, kind: Kind, origin: Vec2, stamp: Stamp, cancelled: bool }
#[derive(Resource, Default)]
struct ModelTouch {
    contact: Option<Contact>,
    pending: Option<(u64, ButtonTarget, Stamp, Vec2)>,
    scroll: Option<(u64, Stamp, i32)>,
    admitted: bool,
}
pub(super) fn cancel_wheel(world:&mut World) {
    if let Some(mut state)=world.get_resource_mut::<ModelTouch>() {
        if state.contact.as_ref().is_some_and(|c|matches!(c.kind,Kind::Knob {..})) {state.contact=None;}
        state.scroll=None;
    }
}
#[derive(SystemParam)]
pub(super) struct Bridge<'w, 's> {
    geometry: Res<'w, Geometry>,
    options: Res<'w, super::pipboy_options::Options>,
    state: ResMut<'w, ModelTouch>,
    pip: Option<Res<'w, PipBoy>>,
    screen: Option<Res<'w, PipBoyScreen>>,
    parts: Query<'w, 's, (&'static Visibility, &'static InheritedVisibility), With<crate::pipboy_arm::PipBoyPart>>,
    paused: Option<Res<'w, nvf_play::pause::Paused>>,
    script: Option<Res<'w, nvf_play::script::ScriptState>>,
    flat: Query<'w, 's, (&'static ComputedNode, &'static UiGlobalTransform), With<nvf_play::pipboy::FlatScreen>>,
    windows: Query<'w, 's, &'static Window, With<bevy::window::PrimaryWindow>>,
    cameras: Query<'w, 's, &'static Camera, With<nvf_play::PlayerCamera>>,
    recognised: Query<'w, 's, (Entity, &'static crate::pipboy_arm::PipBoyPart, Option<&'static Visibility>, Option<&'static InheritedVisibility>)>,
}
fn current(geometry: &Geometry, pip: Option<&PipBoy>, screen: Option<&PipBoyScreen>, size: Vec2) -> bool {
    pip.zip(screen).is_some_and(|(p, s)| p.phase == Phase::Up && p.frame == Frame::Device && s.on_model
        && geometry.image == Some(s.image.id()) && geometry.page == Some(p.page) && geometry.tab == p.tab()
        && geometry.size == size && (!geometry.buttons.is_empty() || geometry.screen.is_some() || geometry.knob.is_some()))
}
impl Bridge<'_, '_> {
    pub fn cancel(&mut self) { self.state.contact = None; self.state.pending = None; self.state.scroll = None; self.state.admitted = false; }
    pub fn prepare(&mut self, allowed: bool, size: Vec2) {
        let modal = self.script.as_ref().and_then(|s| s.rt.as_ref()).is_some_and(|rt| nvf_play::tiles::stats_menu::modal_over(rt.game.menu));
        self.state.admitted = allowed && !self.paused.as_ref().is_some_and(|p| p.0) && !modal && current(&self.geometry, self.pip.as_deref(), self.screen.as_deref(), size)
            && self.geometry.camera.is_some_and(|e|self.cameras.get(e).is_ok_and(|c|c.is_active))
            && self.geometry.entities.iter().all(|e| self.parts.get(*e).is_ok_and(|(v,i)|*v!=Visibility::Hidden && i.get()));
        if !self.options.wheel && self.state.contact.as_ref().is_some_and(|c|matches!(c.kind,Kind::Knob {..})) {
            self.state.contact=None; self.state.scroll=None;
        }
        if !self.state.admitted || self.state.contact.as_ref().is_some_and(|c| c.stamp != self.geometry.stamp()
            || matches!(c.kind, Kind::Button(target) if !self.geometry.buttons.contains(&target))) {
            self.state.contact = None; self.state.pending = None; self.state.scroll = None;
        }
    }
    /// Map raw admission uses the same live device-source guards; its publisher
    /// additionally rejects incomplete current poses before typed edits apply.
    pub(super) fn map_projection_ready(&self)->bool {
        let (Some(pip),Some(screen))=(self.pip.as_deref(),self.screen.as_deref()) else {return false;};
        if pip.frame==Frame::Screen || !screen.on_model {return true;}
        self.state.admitted && self.geometry.screen.is_some()
            && self.recognised.iter().all(|(entity,part,visible,inherited)| {
                !matches!(part,crate::pipboy_arm::PipBoyPart::Screen|crate::pipboy_arm::PipBoyPart::Button(_)|crate::pipboy_arm::PipBoyPart::ScrollKnob)
                    || !visible.is_some_and(|v|*v!=Visibility::Hidden) || !inherited.is_some_and(|v|v.get())
                    || self.geometry.entities.contains(&entity)
            })
    }
    /// Missing/stale geometry never becomes an unclaimed closing-handle region.
    pub fn closing_unclaimed(&self, position: Vec2) -> bool {
        let (Some(pip),Some(screen),Ok(window))=(self.pip.as_deref(),self.screen.as_deref(),self.windows.single()) else {return false};
        if !position.is_finite() || pip.phase!=Phase::Up || !window.focused {return false;}
        if pip.frame==Frame::Screen || !screen.on_model {
            let Ok((node,at))=self.flat.single() else {return false};
            if !node.size().is_finite() || node.size().min_element()<=0.0 || !at.translation.is_finite() || !window.scale_factor().is_finite() || window.scale_factor()<=0.0 {return false;}
            return !Rect::from_center_size(at.translation,node.size()).contains(position*window.scale_factor());
        }
        if !current(&self.geometry,Some(pip),Some(screen),window.resolution.size())
            || self.geometry.screen.is_none()
            || !self.geometry.camera.is_some_and(|e|self.cameras.get(e).is_ok_and(|c|c.is_active))
            || !self.geometry.entities.iter().all(|e|self.parts.get(*e).is_ok_and(|(v,i)|*v!=Visibility::Hidden && i.get())) {return false;}
        if self.recognised.iter().any(|(entity,part,visible,inherited)| {
            matches!(part,crate::pipboy_arm::PipBoyPart::Screen|crate::pipboy_arm::PipBoyPart::Button(_)|crate::pipboy_arm::PipBoyPart::ScrollKnob)
                && visible.is_none_or(|v|*v!=Visibility::Hidden) && inherited.is_none_or(|i|i.get()) && !self.geometry.entities.contains(&entity)
        }) {return false;}
        !self.geometry.screen.is_some_and(|r|r.contains(position))
            && !self.geometry.buttons.iter().any(|b|b.bounds.contains(position))
            && !self.geometry.knob.is_some_and(|r|r.contains(position))
    }
    pub(super) fn closing_region_unclaimed(&self, rect:Rect)->bool {
        if !rect.min.is_finite() || !rect.max.is_finite() || rect.size().min_element()<=0.0 || !self.closing_unclaimed(rect.center()) {return false;}
        let overlaps=|r:Rect| {let i=r.intersect(rect);i.width()>0.0 && i.height()>0.0};
        if self.pip.as_deref().zip(self.screen.as_deref()).is_some_and(|(p,s)|p.frame==Frame::Screen || !s.on_model) {
            let Ok((node,at))=self.flat.single() else {return false;};
            let Ok(window)=self.windows.single() else {return false;};
            return !overlaps(Rect::from_center_size(at.translation/window.scale_factor(),node.size()/window.scale_factor()));
        }
        !self.geometry.screen.is_some_and(overlaps) && !self.geometry.knob.is_some_and(overlaps)
            && !self.geometry.buttons.iter().any(|b|overlaps(b.bounds))
    }
    pub fn event(&mut self, event: &TouchInput, unclaimed: bool) -> bool {
        if let Some(contact) = self.state.contact.as_mut().filter(|c| c.id == event.id) {
            contact.cancelled |= !event.position.is_finite() || contact.stamp != self.geometry.stamp() || event.phase == TouchPhase::Canceled;
            let mut lines = 0;
            match &mut contact.kind {
                Kind::Button(target) => contact.cancelled |= event.position.distance(contact.origin)>12.0 || self.geometry.target(event.position)!=Some(*target),
                Kind::Knob { previous, remainder } if !contact.cancelled => {
                    let delta = event.position.y - previous.y; *previous = event.position;
                    *remainder += delta; lines = (*remainder / 20.0).trunc().clamp(-12.0,12.0) as i32;
                    *remainder -= lines as f32 * 20.0;
                }
                _ => {}
            }
            let update = (!contact.cancelled && lines != 0).then(|| (contact.id,contact.stamp.clone(),lines));
            if contact.cancelled { self.state.scroll = None; }
            else if let Some((id,stamp,lines))=update {
                let old=self.state.scroll.take().filter(|(old,s,_)|*old==id && *s==stamp).map_or(0,|(_,_,n)|n);
                self.state.scroll=Some((id,stamp,(old+lines).clamp(-12,12)));
            }
            if matches!(event.phase, TouchPhase::Ended | TouchPhase::Canceled) {
                let contact = self.state.contact.take().unwrap();
                if !contact.cancelled && event.phase == TouchPhase::Ended && self.state.admitted {
                    if let Kind::Button(target)=contact.kind { self.state.pending = Some((event.id,target,contact.stamp,event.position)); }
                }
            }
            return true;
        }
        if self.state.contact.is_some() || self.state.pending.is_some() || self.state.scroll.is_some() { return true; }
        if !self.state.admitted || !unclaimed || event.phase != TouchPhase::Started { return false; }
        let kind=if let Some(target)=self.geometry.target(event.position) {Kind::Button(target)}
            else if self.options.wheel && event.position.is_finite() && !self.geometry.on_screen(event.position) && self.geometry.knob.is_some_and(|r|r.contains(event.position)) {Kind::Knob {previous:event.position,remainder:0.0}}
            else {return false};
        self.state.contact = Some(Contact { id: event.id, kind, origin: event.position, stamp: self.geometry.stamp(), cancelled: false });
        true
    }
}
#[allow(clippy::too_many_arguments)]
fn dispatch(mut commands: Commands, mut state: ResMut<ModelTouch>, geometry: Res<Geometry>, pip: Option<Res<PipBoy>>, screen: Option<Res<PipBoyScreen>>, admission: nvf_play::pipboy::touch_items::Admission, script: Option<Res<nvf_play::script::ScriptState>>, paused: Option<Res<nvf_play::pause::Paused>>, physical: Option<Res<nvf_play::pipboy::touch_scroll::PhysicalPriority>>, options: Res<super::pipboy_options::Options>, parts: Query<(&Visibility,&InheritedVisibility), With<crate::pipboy_arm::PipBoyPart>>, cameras: Query<&Camera,With<nvf_play::PlayerCamera>>, mut out: MessageWriter<PipBoyInput>) {
    if !options.wheel && state.contact.as_ref().is_some_and(|c|matches!(c.kind,Kind::Knob {..})) {state.contact=None;}
    let pending=state.pending.take(); let scroll=state.scroll.take();
    let owner=state.contact.as_ref().map(|c|c.id).or_else(||pending.as_ref().map(|(id,..)|*id)).or_else(||scroll.as_ref().map(|(id,..)|*id));
    let modal = script.as_ref().and_then(|s| s.rt.as_ref()).is_some_and(|rt| nvf_play::tiles::stats_menu::modal_over(rt.game.menu));
    let allowed=state.admitted && admission.allows_contact(owner) && !modal && !paused.as_ref().is_some_and(|p| p.0) && !physical.as_ref().is_some_and(|p| p.active)
        && geometry.camera.is_some_and(|e|cameras.get(e).is_ok_and(|c|c.is_active))
        && geometry.entities.iter().all(|e|parts.get(*e).is_ok_and(|(v,i)|*v!=Visibility::Hidden && i.get())) && current(&geometry,pip.as_deref(),screen.as_deref(),geometry.size);
    if !allowed { state.contact=None; state.admitted=false; return; }
    if let Some((id,target,stamp,at))=pending {
        if admission.allows_contact(Some(id)) && geometry.stamp()==stamp && geometry.target(at)==Some(target) {out.write(PipBoyInput::Page(target.page));}
    }
    if let Some((id,stamp,lines))=scroll {
        if options.wheel && geometry.stamp()==stamp && admission.allows_contact(Some(id)) {
            if let Some(request)=pip.as_deref().zip(screen.as_deref()).and_then(|(p,s)|nvf_play::pipboy::scrollbar::ListScrollRequest::new(p,s,nvf_play::pipboy::scrollbar::ScrollBy::Lines(lines))) {
                let request=request.with_contact(id); commands.queue(move |world:&mut World| {request.apply(world);});
            }
        }
    }
}
#[derive(Component)]
struct Visual;
#[derive(Component)]
struct VisualButton(Page);
#[derive(Component)]
struct VisualCaption;
type VisualState=(Geometry,Option<Page>,bool,Option<super::pipboy_options::PaintFrame>);
fn draw(mut commands: Commands, geometry: Res<Geometry>, state: Res<ModelTouch>, old: Query<Entity, With<Visual>>, canvas:super::pipboy_options::OverlayCanvas, mut previous: Local<Option<VisualState>>) {
    let pressed = state.contact.as_ref().filter(|c| !c.cancelled).and_then(|c|if let Kind::Button(t)=c.kind {Some(t.page)}else{None});
    let frame=canvas.frame();
    let next = (geometry.clone(), pressed, state.admitted,frame);
    if previous.as_ref() == Some(&next) { return; }
    *previous = Some(next);
    for e in &old { commands.entity(e).despawn(); }
    let Some(frame)=frame.filter(|_|state.admitted) else {return;};
    for button in &geometry.buttons {
        let pressed = pressed == Some(button.page);
        let r = button.bounds;
        commands.spawn((Visual, VisualButton(button.page), UiTargetCamera(frame.camera), Node { position_type: PositionType::Absolute, left: px(r.min.x/frame.scale), top: px(r.min.y/frame.scale), width: px(r.width()/frame.scale), height: px(r.height()/frame.scale), border: UiRect::all(px((if pressed { 3.0 } else { 1.0 })/frame.scale)), align_items: AlignItems::Center, justify_content: JustifyContent::Center, ..default() },
            BorderColor::all(nvf_play::interface::pipboy_amber(255.0).with_alpha(if pressed { 1.0 } else { 0.55 })),
            BackgroundColor(Color::srgba(0.02, 0.025, 0.02, if pressed { 0.65 } else { 0.2 })), GlobalZIndex(1800), Pickable::IGNORE))
            .with_children(|p| { p.spawn((VisualCaption, Text::new(button.page.title()), TextFont { font_size: FontSize::Px(12.0/frame.scale), ..default() }, TextColor(nvf_play::interface::pipboy_amber(255.0)), Pickable::IGNORE)); });
    }
}
fn paint_graphics(options:Res<super::pipboy_options::Options>,state:Res<ModelTouch>,mut buttons:Query<(&VisualButton,&mut BackgroundColor,&mut BorderColor),With<Visual>>,mut captions:Query<&mut TextColor,With<VisualCaption>>) {
    let pressed=state.contact.as_ref().filter(|c|!c.cancelled).and_then(|c|if let Kind::Button(t)=c.kind {Some(t.page)}else{None});
    let alpha=if options.graphics {1.0} else {0.0};
    for (button,mut background,mut border) in &mut buttons {
        let held=pressed==Some(button.0);
        background.0=Color::srgba(0.02,0.025,0.02,alpha*if held {0.65} else {0.2});
        *border=BorderColor::all(nvf_play::interface::pipboy_amber(255.0).with_alpha(alpha*if held {1.0} else {0.55}));
    }
    for mut caption in &mut captions {caption.0=nvf_play::interface::pipboy_amber(255.0).with_alpha(alpha);}
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<super::pipboy_options::Options>().init_resource::<Geometry>().init_resource::<ModelTouch>().add_message::<PipBoyInput>()
        .add_systems(Update, dispatch.after(nvf_play::pipboy::PointerSet).after(crate::pipboy_arm::show_screen).after(nvf_play::PlaySet::Scripts).before(nvf_play::PlaySet::Ui))
        .add_systems(Update, draw.after(dispatch).before(nvf_play::PlaySet::Ui))
        // Paint after the core Update recolour; these overlay roots are not PipBoyRoot descendants.
        .add_systems(PostUpdate, paint_graphics);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ingress(mut bridge: Bridge, mut events: MessageReader<TouchInput>) {
        bridge.prepare(true, Vec2::new(1280.0, 960.0));
        for event in events.read() { bridge.event(event, true); }
    }
    fn scene(density: f32) -> App {
        let mut app = crate::pipboy_arm::tests::model_control_fixture();
        let world = app.world_mut();
        let mut windows = world.query::<&mut Window>();
        let mut window = windows.single_mut(world).unwrap();
        window.resolution = bevy::window::WindowResolution::new((1280.0*density) as u32, (960.0*density) as u32);
        window.resolution.set_scale_factor_override(Some(density));
        let mut cameras = world.query::<&mut Camera>();
        cameras.single_mut(world).unwrap().computed.target_info = Some(bevy::camera::RenderTargetInfo { physical_size: UVec2::new((1280.0*density) as u32, (960.0*density) as u32), scale_factor: density });
        install(&mut app);
        app.add_message::<TouchInput>().add_systems(PreUpdate, ingress);
        for _ in 0..2 { app.update(); }
        app
    }
    fn event(app: &mut App, id: u64, phase: TouchPhase, position: Vec2) {
        app.world_mut().write_message(TouchInput { window: Entity::PLACEHOLDER, id, phase, position, force: None });
        app.update();
    }
    fn settle(app: &mut App) { for _ in 0..3 { app.update(); } }
    #[test]
    fn actual_posed_buttons_publish_aligned_targets_and_touch_switches_once_with_feedback() {
        for density in [1.0, 2.0, 3.0] {
            let mut app = scene(density);
            let geometry = app.world().resource::<Geometry>().clone();
            assert_eq!(geometry.buttons.len(), 3);
            let target = geometry.buttons.iter().find(|b| b.page == Page::Stats).unwrap().bounds;
            assert!(target.width() >= 48.0 && target.height() >= 48.0);
            event(&mut app, 91, TouchPhase::Started, target.center());
            let world = app.world_mut();
            let mut nodes = world.query_filtered::<&Node, With<Visual>>();
            assert!(nodes.iter(world).any(|n| n.left == px(target.min.x) && n.top == px(target.min.y) && n.border == UiRect::all(px(3.0))));
            assert_eq!(world.resource::<PipBoy>().page, Page::Items, "press cannot act");
            event(&mut app, 91, TouchPhase::Ended, target.center()); settle(&mut app);
            assert_eq!(app.world().resource::<PipBoy>().page, Page::Stats);
            assert!(app.world().resource::<ModelTouch>().pending.is_none());
            assert!(app.world().resource::<ModelTouch>().contact.is_none());
        }
    }
    #[test]
    fn model_button_drag_late_capture_and_geometry_change_cancel_without_replay() {
        for cause in 0..3 {
            let mut app = scene(1.0);
            let point = app.world().resource::<Geometry>().buttons.iter().find(|b| b.page == Page::Data).unwrap().bounds.center();
            event(&mut app, 92, TouchPhase::Started, point);
            match cause {
                0 => event(&mut app, 92, TouchPhase::Moved, point + Vec2::X * 20.0),
                1 => { app.insert_resource(nvf_play::pipboy::touch_items::OverlayCapture(true)); }
                _ => { app.world_mut().resource_mut::<Geometry>().size.x += 1.0; }
            }
            event(&mut app, 92, TouchPhase::Ended, point); settle(&mut app);
            assert_eq!(app.world().resource::<PipBoy>().page, Page::Items);
            app.world_mut().remove_resource::<nvf_play::pipboy::touch_items::OverlayCapture>(); settle(&mut app);
            assert_eq!(app.world().resource::<PipBoy>().page, Page::Items, "rejected page requests never replay");
            event(&mut app, 93, TouchPhase::Started, point);
            event(&mut app, 93, TouchPhase::Ended, point); settle(&mut app);
            assert_eq!(app.world().resource::<PipBoy>().page, Page::Data, "a fresh contact still works");
        }
    }
    include!("pipboy_model/safety_tests.rs");

}
