//! Paint-only dialogue themes and an independent explanatory-caption switch.
use bevy::prelude::*;
use crate::settings::{add_choice_list, add_setting, ChoiceList, Setting, Slider};

pub(crate) const THEME_KEY: &str = "dialogue-theme-v1";
pub(crate) const BACKGROUND_KEY: &str = "dialogue-background-opacity-v1";
pub(crate) const FRAME_KEY: &str = "dialogue-frame-opacity-v1";
/// Conservative room for two16-unit lines at1.2line height plus padding/borders.
pub(crate) const MIN_PREVIEW_UNITS: f32 = 78.0;
pub(crate) const HINTS_KEY: &str = "dialogue-hints-v1";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Theme {
    #[default] Minimal, ClassicGlass, SplitPanel, RetroConsole, WristTerminal,
}
impl Theme {
    const ALL: [Self; 5] = [Self::Minimal, Self::ClassicGlass, Self::SplitPanel, Self::RetroConsole, Self::WristTerminal];
    fn name(self) -> &'static str {
        match self { Self::Minimal => "Minimal", Self::ClassicGlass => "Classic Glass", Self::SplitPanel => "Split Panel", Self::RetroConsole => "Retro Console", Self::WristTerminal => "Wrist Terminal" }
    }
}
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct Appearance {
    pub theme: Theme,
    pub background: f32,
    pub frame: f32,
    pub hints: bool,
}
impl Default for Appearance {
    fn default() -> Self { Self { theme: Theme::Minimal, background: 0.18, frame: 0.60, hints: true } }
}
fn opacity(value: f32) -> f32 { if value.is_finite() { value.clamp(0.0, 1.0) } else { 0.0 } }
fn backing(value: f32) -> f32 { opacity(value).min(0.80) }
fn shown(value: f32) -> String { format!("{:.0}%", opacity(value) * 100.0) }
fn shown_backing(value: f32) -> String { shown(opacity(value)*0.80) }
fn chosen(world: &World) -> String { world.resource::<Appearance>().theme.name().into() }
fn pick(world: &mut World, name: &str) -> bool {
    let Some(theme) = Theme::ALL.into_iter().find(|t| t.name() == name) else { return false; };
    world.resource_mut::<Appearance>().theme = theme;
    true
}
const THEME_NOTE: &str = "Thin dialogue frames using your HUD colour. The live sample changes here; themes keep choices, targets, text sizes and speaker-side framing.";
fn theme_setting(note: &'static str) -> Setting {
    Setting { key: THEME_KEY, group: crate::pause::GROUP_MENUS, label: "Dialogue theme",
        note, switches: "", get: |_|false, set: |_,_|{}, slider: None }
}
fn theme_choices() -> ChoiceList {
    ChoiceList { names: |_|Theme::ALL.iter().map(|t|t.name().into()).collect(), chosen, pick, save: chosen, load: pick, ask_keep: false }
}
/// Register the real choices/callbacks with an authored note before the full installer.
/// A pre-existing ordinary row would intentionally prevent list registration.
#[cfg(test)]
pub(crate) fn register_theme_note(app: &mut App, note: &'static str) {
    app.init_resource::<Appearance>();
    add_choice_list(app, theme_setting(note), theme_choices());
}
pub(super) fn install(app: &mut App) {
    app.init_resource::<Appearance>().add_systems(Update, paint.after(crate::PlaySet::Ui).after(crate::ui_colour::Recolour).before(crate::PlaySet::Sync));
    add_choice_list(app, theme_setting(THEME_NOTE), theme_choices());
    for (key,label,frame) in [(BACKGROUND_KEY,"Dialogue background opacity",false),(FRAME_KEY,"Dialogue frame opacity",true)] {
        let slider = if frame {
            Slider { get: |w|w.resource::<Appearance>().frame, set: |w,v|w.resource_mut::<Appearance>().frame=opacity(v), min:0.0, step:0.01, shown:Some(shown) }
        } else {
            Slider { get: |w|w.resource::<Appearance>().background/0.80, set: |w,v|w.resource_mut::<Appearance>().background=backing(opacity(v)*0.80), min:0.0, step:0.01, shown:Some(shown_backing) }
        };
        add_setting(app, Setting { key, group:crate::pause::GROUP_MENUS,label,
            note:"Live appearance only. Backing is capped at 80% so the scene remains visible. Text and selection cues keep their contrast; targets and input stay unchanged.",switches:"",get:|_|false,set:|_,_|{},slider:Some(slider) });
    }
    add_setting(app, Setting {key:HINTS_KEY,group:crate::pause::GROUP_MENUS,label:"Show dialogue hints",
        note:"Show gesture and tutorial captions in enhanced dialogue. NPC speech, choices, requirements and selected-action labels always remain visible.",switches:"",
        get:|w|w.resource::<Appearance>().hints,set:|w,v|w.resource_mut::<Appearance>().hints=v,slider:None});
}

/// All theme decoration is paint-only: never Button, Action or an input owner.
#[derive(Component, Clone, Copy)]
pub(super) enum Paint {
    Panel, Speaker, Speech, Hint, ChoiceText { darkened: bool },
    Choice { index: usize, short: bool }, Focus { index: usize, short: bool }, Confirm, Caption, PreviewSelected, PreviewFocus, Trim(u8),
}
#[derive(Component)]
pub(crate) struct Preview;
// Existing unmet-check feedback is semantic, not a selectable theme colour.
pub(super) fn check_line() -> Color { Color::srgb(0.8,0.2,0.15) }
fn palette(rgb: [u8;3], brightness: f32, alpha: f32) -> Color {
    crate::interface::tint(rgb,brightness).with_alpha(opacity(alpha))
}
impl Appearance {
    fn line(self, rgb:[u8;3], factor:f32) -> Color { palette(rgb,255.0,self.frame*factor) }
    fn fill(self, rgb:[u8;3], brightness:f32, factor:f32) -> Color { palette(rgb,brightness,backing(self.background)*factor) }
    fn colors(self, kind:Paint, selected:usize, rgb:[u8;3]) -> (Option<Color>,Option<Color>,Option<Color>) {
        match kind {
            Paint::Panel => (None,Some(self.fill(rgb,match self.theme {Theme::Minimal=>12.0,Theme::ClassicGlass=>20.0,Theme::SplitPanel=>12.0,Theme::RetroConsole=>8.0,Theme::WristTerminal=>16.0},1.0)),Some(self.line(rgb,1.0))),
            Paint::Speaker | Paint::Speech | Paint::Caption => (Some(palette(rgb,255.,1.)),None,None),
            Paint::ChoiceText{darkened} => (Some(palette(rgb,if darkened {160.}else{255.},1.)),None,None),
            Paint::Hint => (Some(palette(rgb,190.,1.)),None,None),
            Paint::Choice{index,short} => {
                let on=index==selected;
                (None,Some(self.fill(rgb,if on {48.}else{16.},if on {1.0}else{0.35})),Some(if on && short {check_line().with_alpha(opacity(self.frame))}else{self.line(rgb,if on {1.0}else if short {0.65}else{0.25})}))
            }
            Paint::Focus{index,short} => (None,Some(if index==selected && short {check_line()}else{palette(rgb,255.,if index==selected {1.}else{0.})}),None),
            Paint::PreviewFocus => (None,Some(palette(rgb,255.,1.)),None),
            Paint::Confirm | Paint::PreviewSelected => (None,Some(self.fill(rgb,48.,1.0)),Some(self.line(rgb,1.0))),
            Paint::Trim(n) => {
                let visible=match self.theme {
                    Theme::Minimal => false,
                    Theme::ClassicGlass => n<4,
                    Theme::SplitPanel => n==4 || n==5,
                    Theme::RetroConsole => n<4 || n==6,
                    Theme::WristTerminal => n==0 || n==3 || n>=6,
                };
                (None,Some(self.line(rgb,if visible {1.0}else{0.0})),None)
            }
        }
    }
}
type Painted = (&'static Paint,Option<&'static mut TextColor>,Option<&'static mut BackgroundColor>,Option<&'static mut BorderColor>,Option<&'static mut Node>);
fn paint(
    appearance: Res<Appearance>, menu: Res<super::DialogueMenu>,
    mut nodes: Query<Painted>,
) {
    let rgb=crate::interface::hud_rgb();
    for (kind,text,back,border,node) in &mut nodes {
        let (t,b,line)=appearance.colors(*kind,menu.selected,rgb);
        if let (Some(mut value),Some(color))=(text,t) { if value.0!=color {value.0=color;} }
        if let (Some(mut value),Some(color))=(back,b) { if value.0!=color {value.0=color;} }
        if let (Some(mut value),Some(color))=(border,line) { let next=BorderColor::all(color);if *value!=next {*value=next;} }
        if let (Paint::Hint,Some(mut node))=(kind,node) {
            let display=if appearance.hints {Display::Flex}else{Display::None};
            if node.display!=display {node.display=display;}
        }
    }
}
pub(super) fn decoration(parent:&mut ChildSpawnerCommands) {
    // One-pixel insets/rails, original procedural trim; all share unchanged hit geometry.
    for (n,node) in [
        (0,Node{left:px(4.),top:px(3.),width:px(28.),height:px(1.),..default()}),
        (1,Node{right:px(4.),top:px(3.),width:px(28.),height:px(1.),..default()}),
        (2,Node{left:px(3.),bottom:px(4.),width:px(1.),height:px(24.),..default()}),
        (3,Node{right:px(3.),bottom:px(4.),width:px(1.),height:px(24.),..default()}),
        (4,Node{left:px(3.),right:px(3.),top:px(28.),height:px(1.),..default()}),
        (5,Node{left:px(3.),top:px(3.),bottom:px(3.),width:px(1.),..default()}),
        (6,Node{left:px(8.),top:px(6.),width:px(12.),height:px(1.),..default()}),
        (7,Node{left:px(24.),top:px(6.),width:px(6.),height:px(1.),..default()}),
    ] {
        parent.spawn((Paint::Trim(n),bevy::picking::Pickable::IGNORE,Node{position_type:PositionType::Absolute,..node},BackgroundColor(Color::NONE)));
    }
}

pub(crate) fn is_row(key:&str)->bool { [THEME_KEY,BACKGROUND_KEY,FRAME_KEY,HINTS_KEY].contains(&key) }
/// The pause owner supplies its actual clipped note-region Node; never a gameplay panel.
pub(crate) fn preview(parent:&mut ChildSpawnerCommands,node:Node) {
    parent.spawn((Preview,Paint::Panel,node,bevy::picking::Pickable::IGNORE,BackgroundColor(Color::NONE),BorderColor::all(Color::NONE)))
        .with_children(|p| {
            decoration(p);
            p.spawn((Text::new("Dialogue preview"),Paint::Speaker,bevy::picking::Pickable::IGNORE,TextFont{font_size:crate::interface::font_units(16.0),..default()},TextColor(crate::interface::tint(crate::interface::hud_rgb(),255.))));
            p.spawn((Paint::PreviewSelected,Node{width:percent(100.),min_height:percent(45.),border:UiRect::all(px(1.)),padding:UiRect::horizontal(px(3.)),..default()},BackgroundColor(Color::NONE),BorderColor::all(Color::NONE),bevy::picking::Pickable::IGNORE))
                .with_children(|p| {p.spawn((Paint::PreviewFocus,bevy::picking::Pickable::IGNORE,Node{position_type:PositionType::Absolute,left:px(1.),top:percent(50.),width:px(2.),height:px(4.),..default()},BackgroundColor(crate::interface::tint(crate::interface::hud_rgb(),255.))));p.spawn((Text::new("Selected response"),Paint::Caption,bevy::picking::Pickable::IGNORE,TextFont{font_size:crate::interface::font_units(16.0),..default()},TextColor(crate::interface::tint(crate::interface::hud_rgb(),255.))));});
        });
}

#[cfg(test)]
mod tests;
