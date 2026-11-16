//! Independent sizes for existing trading, repair and crafting menus.
use crate::settings::{self, Setting, Slider};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind { Barter, Repair, Workbench }

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct MenuReadability {
    pub barter: f32,
    pub repair: f32,
    pub workbench: f32,
}
impl Default for MenuReadability {
    fn default() -> Self { Self { barter: 1.0, repair: 1.0, workbench: 1.0 } }
}
fn fit(value: f32) -> f32 {
    if !value.is_finite() { return 1.0; }
    (value.clamp(0.5, 1.5) * 20.0).round() / 20.0
}
fn part(value: f32) -> f32 { fit(value) - 0.5 }
fn of_part(value: f32) -> f32 { fit(0.5 + value) }
fn show(value: f32) -> String { format!("{:.0}%", of_part(value) * 100.0) }
impl MenuReadability {
    pub fn size(self, kind: MenuKind) -> f32 {
        fit(match kind { MenuKind::Barter => self.barter, MenuKind::Repair => self.repair,
            MenuKind::Workbench => self.workbench })
    }
}

/// Each consumer lays out its existing rows with this size, retaining viewport clipping.
pub fn register(app: &mut App, group: &'static str) {
    app.init_resource::<MenuReadability>();
    settings::add_setting(app, Setting::slider("barter-menu-size-v1", group, "Buying and selling size",
        "Text and row size in the normal buying and selling menus. Other menus keep their own sizes.",
        Slider::choice(21, |w| part(w.resource::<MenuReadability>().barter),
            |w,v| w.resource_mut::<MenuReadability>().barter = of_part(v), show)));
    settings::add_setting(app, Setting::slider("repair-menu-size-v1", group, "Repair menu size",
        "Text and row size in repair menus. Buying, selling and workbenches keep their own sizes.",
        Slider::choice(21, |w| part(w.resource::<MenuReadability>().repair),
            |w,v| w.resource_mut::<MenuReadability>().repair = of_part(v), show)));
    settings::add_setting(app, Setting::slider("workbench-menu-size-v1", group, "Workbench menu size",
        "Text and row size at workbenches, campfires and reloading benches. Other menus keep their own sizes.",
        Slider::choice(21, |w| part(w.resource::<MenuReadability>().workbench),
            |w,v| w.resource_mut::<MenuReadability>().workbench = of_part(v), show)));
}

#[cfg(test)]
#[path = "menu_readability_tests.rs"]
mod tests;
