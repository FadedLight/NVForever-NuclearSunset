//! One bounded handoff from the existing pre-input reader to post-selector menus.
//! The leaf bridges never read a second raw stream or mutate physical input state.
use super::*;
use bevy::{camera::{NormalizedRenderTarget, RenderTarget}, ecs::{entity::ContainsEntity, system::SystemParam}, input::touch::Touches, ui::DefaultUiCamera};
use nvf_play::{maintenance_touch, menu::{barter_touch, model::Mode}, pipboy::{Page, Phase}};

const MAX_EDGES: usize = 128;
const MAX_IDS: usize = 32;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind { Barter, Repair, Mod, Vendor }
#[derive(Clone, Copy, PartialEq)]
struct Stamp { window: Entity, physical: UVec2, density: f32, safe: nvf_play::touch_safe_area::SafeInsets, scale: f32 }
#[derive(Resource, Default)]
pub(super) struct Batch {
    kind: Option<Kind>, stamp: Option<Stamp>, allowed: bool,
    events: Vec<TouchInput>, ids: [Option<u64>; MAX_IDS], overflow: bool,
}
impl Batch {
    fn owns(&self, id: u64) -> bool { self.ids.contains(&Some(id)) }
    fn record(&mut self, event: &TouchInput) {
        if !self.owns(event.id) {
            if let Some(slot) = self.ids.iter_mut().find(|slot| slot.is_none()) { *slot = Some(event.id); }
            else { self.overflow = true; }
        }
        if self.events.len() < MAX_EDGES && !self.overflow { self.events.push(*event); }
        else { self.overflow = true; self.events.clear(); }
    }
}

#[derive(SystemParam)]
struct Sources<'w> {
    active: Option<Res<'w, nvf_play::menu::ActiveMenu>>,
    barter: Option<Res<'w, barter_touch::Options>>,
    repair: Option<Res<'w, nvf_play::pipboy::repair::PipRepair>>,
    modding: Option<Res<'w, nvf_play::pipboy::item_mod::PipItemMod>>,
    vendor: Option<Res<'w, nvf_play::repair::RepairServices>>,
    pip: Option<Res<'w, nvf_play::pipboy::PipBoy>>,
    script: Option<Res<'w, nvf_play::script::ScriptState>>,
    paused: Option<Res<'w, nvf_play::pause::Paused>>,
    creator: Option<Res<'w, nvf_play::creator::CreatorMenu>>,
    keyboard: Option<Res<'w, nvf_play::bindings::KeyboardCapture>>,
    free: Option<Res<'w, nvf_play::free_cam::FreeCam>>,
    safe: Option<Res<'w, nvf_play::touch_safe_area::SafeInsets>>,
    scale: Option<Res<'w, UiScale>>,
}
impl Sources<'_> {
    // Routing prediction only. Each leaf rechecks the actual menu and item/quote
    // inside its effect consumer, after scripts and other menu activity.
    fn kind(&self, maintenance_enabled: bool) -> Option<Kind> {
        if self.paused.as_ref().is_some_and(|p| p.0) || self.creator.as_ref().is_some_and(|c| c.is_open())
            || self.keyboard.as_ref().is_some_and(|k| k.0) || self.free.as_ref().is_some_and(|f| f.active) { return None; }
        let model = self.active.as_ref().and_then(|a| a.0.as_ref());
        let kind = if model.is_some_and(|m| matches!(m.mode, Mode::Barter(_))) && self.barter.as_ref().is_some_and(|o| o.enabled) {
            Kind::Barter
        } else if maintenance_enabled && self.vendor.as_ref().is_some_and(|v| v.0.is_some()) { Kind::Vendor }
        else if maintenance_enabled && self.pip.as_ref().is_some_and(|p| p.phase == Phase::Up && p.page == Page::Items)
            && model.is_some_and(|m| m.mode == Mode::Inventory && m.quantity.is_none()) {
            if self.repair.as_ref().is_some_and(|r| r.is_open()) { Kind::Repair }
            else if self.modding.as_ref().is_some_and(|m| m.is_open()) { Kind::Mod }
            else { return None; }
        } else { return None; };
        if let Some(rt) = self.script.as_ref().and_then(|s| s.rt.as_ref()) {
            use fnv_game::game::menus;
            let game = &rt.game;
            if game.loading || matches!(game.side_menu(), Some(menus::LOADING | menus::VATS)) { return None; }
            let own = match kind {
                Kind::Barter => game.top_menu().is_none_or(|c| c == menus::BARTER || (c == menus::QUANTITY && model.is_some_and(|m| m.quantity.is_some())))
                    && game.ui_prompt.is_none_or(|c| c == menus::QUANTITY),
                Kind::Repair => game.open_menu_code().is_none_or(|c| matches!(c, menus::INVENTORY | menus::REPAIR)),
                Kind::Mod => game.open_menu_code().is_none_or(|c| matches!(c, menus::INVENTORY | menus::ITEM_MOD)),
                Kind::Vendor => game.open_menu_code().is_none_or(|c| c == fnv_game::repair::REPAIR_SERVICES_MENU),
            };
            if !own { return None; }
        }
        Some(kind)
    }
    fn stamp(&self, entity: Entity, window: &Window) -> Stamp {
        Stamp { window: entity, physical: UVec2::new(window.physical_width(), window.physical_height()), density: window.scale_factor(),
            safe: self.safe.as_deref().copied().unwrap_or_default(), scale: self.scale.as_deref().map_or(1.0, |s| s.0) }
    }
}

#[derive(SystemParam)]
pub(super) struct Ingress<'w> {
    batch: ResMut<'w, Batch>, sources: Sources<'w>, touches: Res<'w, Touches>,
    maintenance: Res<'w, maintenance_touch::State>,
}
impl Ingress<'_> {
    pub fn reset(&mut self) {
        self.batch.events.clear(); self.batch.kind = None; self.batch.stamp = None; self.batch.allowed = false;
        if self.touches.iter().next().is_none() && !self.touches.any_just_released() && !self.touches.any_just_canceled() {
            self.batch.ids.fill(None); self.batch.overflow = false;
        }
    }
    pub fn prepare(&mut self, entity: Entity, window: &Window, allowed: bool) -> bool {
        self.batch.kind = allowed.then(|| self.sources.kind(self.maintenance.enabled())).flatten();
        self.batch.stamp = Some(self.sources.stamp(entity, window)); self.batch.allowed = allowed;
        self.batch.kind.is_some()
    }
    pub fn cancel(&mut self) { self.batch.allowed = false; self.batch.events.clear(); }
    pub fn capture(&mut self, events: &mut MessageReader<TouchInput>) -> bool {
        if self.batch.kind.is_none() { return false; }
        for event in events.read() {
            if self.batch.stamp.is_some_and(|s| s.window == event.window) { self.batch.record(event); }
        }
        true
    }
    pub fn tail(&mut self, event: &TouchInput) -> bool {
        if !self.batch.owns(event.id) { return false; }
        self.batch.record(event); true
    }
}

#[derive(SystemParam)]
struct Gates<'w> {
    bindings: Option<Res<'w, Bindings>>, platform: Option<Res<'w, nvf_play::pad::PlatformInputGate>>,
    switching: Option<Res<'w, nvf_play::pad::TouchDeviceSwitch>>,
    paused: Option<Res<'w, nvf_play::pause::Paused>>, fresh: Option<Res<'w, nvf_play::menu::FreshMenu>>,
    keyboard: Option<Res<'w, nvf_play::bindings::KeyboardCapture>>, free: Option<Res<'w, nvf_play::free_cam::FreeCam>>,
    creator: Option<Res<'w, nvf_play::creator::CreatorMenu>>,
}
impl Gates<'_> {
    fn allows(&self) -> bool {
        self.bindings.as_ref().is_none_or(|b| b.device != nvf_play::bindings::InputDevice::Controller)
            && self.platform.as_ref().is_none_or(|g| g.allows_actions())
            && self.paused.as_ref().is_none_or(|p| !p.0)
            && self.keyboard.as_ref().is_none_or(|k| !k.0)
            && self.free.as_ref().is_none_or(|f| !f.active)
            && self.creator.as_ref().is_none_or(|c| !c.is_open())
    }
}

#[derive(SystemParam)]
struct Cameras<'w, 's> {
    default: DefaultUiCamera<'w, 's>,
    cameras: Query<'w, 's, (Entity, &'static Camera, &'static RenderTarget, Has<Camera2d>)>,
}
impl Cameras<'_, '_> {
    fn select(&self, window: Entity, physical: UVec2, logical: Vec2) -> Option<Entity> {
        let valid = |camera: &Camera, target: &RenderTarget| camera.is_active
            && camera.physical_target_size() == Some(physical)
            && camera.physical_viewport_rect() == Some(URect::from_corners(UVec2::ZERO, physical))
            && camera.logical_viewport_rect() == Some(Rect::from_corners(Vec2::ZERO, logical))
            && matches!(target.normalize(Some(window)), Some(NormalizedRenderTarget::Window(w)) if w.entity() == window);
        self.default.get().filter(|&entity| self.cameras.get(entity).is_ok_and(|(_, c, t, _)| valid(c, t)))
            .or_else(|| self.cameras.iter().filter(|(_, c, t, two_d)| *two_d && valid(c, t))
                .max_by_key(|(entity, c, _, _)| (c.order, std::cmp::Reverse(entity.to_bits()))).map(|v| v.0))
    }
}

#[allow(clippy::too_many_arguments)]
fn replay(
    mut batch: ResMut<Batch>, state: Res<TouchState>, sources: Sources, gates: Gates, cameras: Cameras,
    windows: Query<(Entity, &Window), With<PrimaryWindow>>, touches: Res<Touches>,
    mut barter: barter_touch::Bridge, mut maintenance: maintenance_touch::Bridge,
    mut ownership: ResMut<nvf_play::ui_pointer::TouchUiCapture>, mut editor: ResMut<super::entrypoints::Session>,
    mut modal: ResMut<super::modal::Capture>,
    (mut vats, mut model): (super::vats_targeting::Bridge, super::pipboy_model::Bridge),
    mut arm: ResMut<super::pipboy_drag::State>,
    mut items: ResMut<nvf_play::pipboy::touch_items::TouchItems>,
    mut map: ResMut<nvf_play::pipboy::data::touch_map::TouchMap>,
) {
    ownership.begin_frame();
    arm.publish_mask(&mut ownership, &touches);
    let current = sources.kind(maintenance.enabled());
    let context = windows.single().ok().and_then(|(entity, window)| {
        let stamp = sources.stamp(entity, window);
        let camera = cameras.select(entity, stamp.physical, window.resolution.size())?;
        Some((stamp, window, camera))
    });
    let consumed = gates.switching.as_ref().is_some_and(|s| !s.allows_actions());
    let fresh = consumed || gates.fresh.as_ref().is_some_and(|f| f.0);
    vats.after_switch(state.hardware_intent, consumed && gates.allows());
    let allowed = batch.allowed && !batch.overflow && !state.suspended && gates.allows()
        && state.physical_keys.is_empty() && state.physical_mouse.is_empty()
        && batch.kind == current && context.is_some_and(|(s, w, _)| batch.stamp == Some(s) && w.focused);
    let neutral = touches.iter().next().is_none() && !touches.any_just_released() && !touches.any_just_canceled();
    model.observe_neutral(neutral);
    editor.observe_neutral(neutral);
    editor.publish(&mut ownership);
    modal.publish(&mut ownership, neutral);
    maintenance.observe_neutral(neutral);
    if let Some((stamp, window, camera)) = context {
        barter.prepare(barter_touch::Frame { window: stamp.window, camera, size: window.resolution.size(), safe: stamp.safe,
            ui_scale: stamp.scale, density: stamp.density, admitted: allowed && current == Some(Kind::Barter), fresh, neutral });
        maintenance.prepare(Some(maintenance_touch::Context::from_window(stamp.window, camera, window, stamp.safe, stamp.scale)),
            allowed && !fresh && matches!(current, Some(Kind::Repair | Kind::Mod | Kind::Vendor)));
    } else { barter.cancel(); maintenance.cancel(); }
    if state.hardware_intent { barter.hardware_intent(); maintenance.hardware_intent(); map.cancel(); }
    else if consumed && gates.allows() { barter.restore_touch(); maintenance.restore_touch(); }
    let mut events = std::mem::take(&mut batch.events);
    for event in &events {
        // Physical hardware or the already-consumed selector edge wins this batch.
        if !allowed || fresh || state.hardware_intent { continue; }
        if current == Some(Kind::Barter) || barter.captures_touch(event.id) { barter.event(event); }
        else { maintenance.event(event, gates.platform.as_ref().is_none_or(|g| g.allows_touch(event.id))); }
    }
    events.clear(); batch.events = events;
    for id in batch.ids.iter().flatten() { ownership.block(*id); }
    let primary = windows.single().ok().map(|(entity,_)|entity);
    for touch in touches.iter().chain(touches.iter_just_pressed()).chain(touches.iter_just_released()).chain(touches.iter_just_canceled()) {
        if barter.captures_touch(touch.id()) || maintenance.owns(touch.id()) || maintenance.captures_touch() || map.captures()
            || vats.owns(touch.id()) || primary.is_some_and(|window|model.owns(window,touch.id())) { ownership.block(touch.id()); }
    }
    if batch.overflow { ownership.quarantine_overflow(); }
    ownership.suppress_pointer(barter.visible() || maintenance.captures_touch() || editor.blocking());
    if current.is_some() { items.cancel(); }
}

pub(super) fn install(app: &mut App) {
    app.add_plugins(maintenance_touch::MaintenanceTouchPlugin)
        .init_resource::<barter_touch::Options>().init_resource::<barter_touch::TouchBarter>()
        .init_resource::<Batch>().init_resource::<nvf_play::ui_pointer::TouchUiCapture>()
        .add_systems(PreUpdate, replay.after(InputSystems).after(nvf_play::pad::InputSwitchSystems::Switch)
            .after(nvf_play::pad::PlatformInputSystems::Remember).after(super::TouchInputSystems)
            .before(nvf_play::pipboy::TouchPointerSet).before(bevy::ui::UiSystems::Focus));
}
