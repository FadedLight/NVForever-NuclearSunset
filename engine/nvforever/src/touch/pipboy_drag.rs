//! Sole-reader adapter: an admitted bottom handle owns one arm contact.
use bevy::{
    ecs::{message::MessageReader, system::SystemParam},
    input::touch::{TouchInput, TouchPhase},
    prelude::*,
};
use nvf_play::{
    pipboy::{gesture::ArmGesture, Frame, Phase, PipBoy, UsePipBoy},
    settings::{self, Setting},
};

#[derive(Resource)]
pub(super) struct Options {
    pub direct: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self { direct: true }
    }
}
#[derive(Clone, Copy)]
struct Drag {
    id: u64,
    origin: Vec2,
    size: Vec2,
    start: f32,
    canceled: bool,
    closing_travel: Option<f32>,
}
// An open handle is already near the bottom edge. Its downward range is
// the actual remaining safe travel, independently of the opening preference.
fn closing_travel(size: Vec2, safe_bottom: f32, origin: Vec2) -> Option<f32> {
    if !size.is_finite()
        || size.min_element() <= 0.0
        || !safe_bottom.is_finite()
        || !origin.is_finite()
        || origin.x < 0.0
        || origin.x > size.x
        || origin.y < 0.0
    {
        return None;
    }
    let travel = size.y - safe_bottom.max(0.0) - origin.y;
    (travel.is_finite() && travel > 0.0).then_some(travel)
}
#[derive(Resource, Default)]
struct State {
    drag: Option<Drag>,
}

#[derive(SystemParam)]
pub(super) struct Bridge<'w, 's> {
    state: ResMut<'w, State>,
    gesture: ResMut<'w, ArmGesture>,
    options: Res<'w, Options>,
    drawer: Res<'w, super::drawer::Options>,
    safe: Res<'w, super::drawer::SafeBottom>,
    pip: Option<Res<'w, PipBoy>>,
    body: Option<Res<'w, crate::player_body::PlayerBody>>,
    use_pip: Option<Res<'w, UsePipBoy>>,
    sim: Option<Res<'w, nvf_play::sim::SimState>>,
    menu: Option<Res<'w, nvf_play::menu::ActiveMenu>>,
    prompt: Option<Res<'w, nvf_play::menu::PromptKeys>>,
    locks: Option<Res<'w, nvf_play::control::ControlLocks>>,
    commands: Commands<'w, 's>,
}
impl Bridge<'_, '_> {
    pub(super) fn captures(&self) -> bool {
        self.gesture.captures()
    }
    pub(super) fn inventory(&self) -> bool {
        self.menu.as_deref().is_some_and(|m| {
            m.0.as_ref()
                .is_some_and(|m| m.mode == nvf_play::menu::model::Mode::Inventory)
        }) && self
            .pip
            .as_deref()
            .is_some_and(|p| p.frame == Frame::Device && (p.phase == Phase::Up || self.captures()))
    }
    fn block(&mut self) {
        self.commands.queue(|world: &mut World| {
            if let Some(mut gate) = world.get_resource_mut::<nvf_play::pad::PlatformInputGate>() {
                gate.set_arm_blocked(true);
            }
        });
    }
    pub(super) fn cancel(&mut self) {
        self.state.drag = None;
        self.gesture.abort();
        if self.captures() {
            self.block();
        }
    }
    /// The parent supplies lifecycle/modal/device admission before draining its reader.
    pub(super) fn prepare(
        &mut self,
        allowed: bool,
        gate: Option<&nvf_play::pad::PlatformInputGate>,
    ) {
        if !self.captures() {
            self.state.drag = None;
            return;
        }
        let root = self
            .body
            .as_deref()
            .and_then(crate::player_body::PlayerBody::first_person_root);
        if !allowed
            || !self.options.direct
            || !self.drawer.enabled
            || self.drawer.destination != super::drawer::Destination::Inventory
            || self.state.drag.is_some_and(|d| {
                d.closing_travel.is_some()
                    && closing_travel(d.size, self.safe.0, d.origin) != d.closing_travel
            })
            || self.locks.as_ref().is_some_and(|l| l.pipboy)
            || gate.is_none_or(nvf_play::pad::PlatformInputGate::native_blocked)
            || root != self.gesture.model()
            || self
                .body
                .as_deref()
                .is_some_and(crate::player_body::PlayerBody::pipboy_clip_missing)
            || self.use_pip.as_deref().is_none_or(|u| !u.on)
            || self
                .sim
                .as_deref()
                .is_none_or(|s| !nvf_play::pipboy::wears_pipboy(s))
        {
            self.cancel();
        }
    }
    /// Caller must first arbitrate native bottom buttons, tray/editor and scrollbar.
    pub(super) fn begin(
        &mut self,
        event: &TouchInput,
        size: Vec2,
        unclaimed: bool,
        gate: Option<&nvf_play::pad::PlatformInputGate>,
    ) -> bool {
        if self.captures()
            || !unclaimed
            || !self.options.direct
            || !self.drawer.enabled
            || self.locks.as_ref().is_some_and(|l| l.pipboy)
            || self.drawer.destination != super::drawer::Destination::Inventory
            || event.phase != TouchPhase::Started
            || !event.position.is_finite()
            || gate.is_none_or(|g| !g.allows_touch(event.id))
            || !super::drawer::handle(size, self.safe.0).contains(event.position)
        {
            return false;
        }
        let Some(pip) = self.pip.as_deref().filter(|p| {
            p.frame == Frame::Device
                && p.warming.is_none()
                && matches!(p.phase, Phase::Down | Phase::Up)
        }) else {
            return false;
        };
        if self.use_pip.as_deref().is_none_or(|u| !u.on)
            || self
                .sim
                .as_deref()
                .is_none_or(|s| !nvf_play::pipboy::wears_pipboy(s))
        {
            return false;
        }
        let Some(model) = self
            .body
            .as_deref()
            .and_then(crate::player_body::PlayerBody::first_person_root)
        else {
            return false;
        };
        if self
            .body
            .as_deref()
            .is_some_and(crate::player_body::PlayerBody::pipboy_clip_missing)
        {
            return false;
        }
        let menu_open = self.menu.as_deref().is_some_and(|m| {
            m.0.as_ref()
                .is_some_and(|m| m.mode == nvf_play::menu::model::Mode::Inventory)
        });
        if (pip.phase == Phase::Up) != menu_open
            || self
                .prompt
                .as_deref()
                .zip(self.menu.as_deref())
                .is_some_and(|(p, m)| p.held(m))
        {
            return false;
        }
        let open = pip.phase == Phase::Up;
        let closing_travel = if open {
            let Some(travel) = closing_travel(size, self.safe.0, event.position) else {
                return false;
            };
            Some(travel)
        } else {
            None
        };
        if !self.gesture.begin(open, model) {
            return false;
        }
        self.state.drag = Some(Drag {
            id: event.id,
            origin: event.position,
            size,
            start: if open { 1.0 } else { 0.0 },
            canceled: false,
            closing_travel,
        });
        self.block();
        true
    }
    pub(super) fn event(&mut self, event: &TouchInput, size: Vec2) {
        let Some(mut drag) = self.state.drag.filter(|d| d.id == event.id) else {
            return;
        };
        let delta = event.position - drag.origin;
        drag.canceled |= drag.size != size || !event.position.is_finite() || delta.x.abs() > 32.0;
        // A changing inset invalidates the captured safe range, just as resize
        // already invalidates its window. Opening keeps its original policy.
        drag.canceled |= drag.closing_travel.is_some()
            && closing_travel(size, self.safe.0, drag.origin) != drag.closing_travel;
        if drag.canceled {
            self.gesture.cancel();
        } else {
            self.gesture.drag(
                drag.start
                    - delta.y
                        / drag
                            .closing_travel
                            .unwrap_or(self.drawer.threshold.max(48.0)),
            );
        }
        if matches!(event.phase, TouchPhase::Ended | TouchPhase::Canceled) {
            if event.phase == TouchPhase::Canceled || drag.canceled {
                self.gesture.cancel();
            } else {
                self.gesture.release();
            }
            self.state.drag = None;
        } else {
            self.state.drag = Some(drag);
        }
    }
    /// Active owner only; other contacts remain quarantined by the shared gate.
    pub(super) fn capture_batch(
        &mut self,
        events: &mut MessageReader<TouchInput>,
        window: Entity,
        size: Vec2,
    ) -> bool {
        if !self.captures() {
            return false;
        }
        for event in events.read() {
            if event.window == window {
                self.event(event, size);
            }
        }
        self.block();
        true
    }
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<Options>()
        .init_resource::<State>()
        .init_resource::<ArmGesture>();
    settings::add_setting(
        app,
        Setting {
            key: "touch-direct-pipboy-v1",
            group: "Controls",
            label: "Direct Pip-Boy swipe",
            note: "With the bottom swipe enabled, drag the actual arm up or down. Release settles open or closed. Cancel restores its starting state; screen-only menus keep the usual swipe.",
            switches: "",
            get: |w| w.resource::<Options>().direct,
            set: |w, v| {
                w.resource_mut::<Options>().direct = v;
                w.resource_mut::<ArmGesture>().abort();
            },
            slider: None,
        },
    );
}

#[cfg(test)]
mod tests;
