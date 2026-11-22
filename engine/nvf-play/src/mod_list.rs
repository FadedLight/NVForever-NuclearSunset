//! The Mod List: the pause menu's "Mod List" entry opens a panel over the world listing every
//! plugin of the load order with what NVForever made of it, so a player sees in the game what
//! is enabled and whether it works [design: owner 2026-11-20, "a mod manager menu that lets
//! you see what is enabled and working in game"].
//!
//! Each row is one plugin, in load order, with a status:
//! - **OK**: its scripts all run (from their source, or from their compiled bytes when the
//!   source is stale) and name nothing NVForever lacks; or it has no scripts (**data**).
//! - **Partial**: it runs, but calls functions or names NVForever does not have yet (those
//!   calls do nothing), or holds extender bytes with no source to compile.
//! The details under the list name the functions and names, and the scripts that run from
//! their bytes. The facts are the ones `crate::mod_log` prints (`GameData::source_units`).
//! Keys: Up/Down, Page Up/Down, Home/End to move; Esc closes.

use crate::script::ScriptState;
use crate::ui_pointer::PointerState;
use crate::{PlaySet, PlayState};
use bevy::prelude::*;
use fnv_game::source_compile::SourceUnit;
use std::collections::{BTreeMap, BTreeSet};

/// Rows shown at once.
const SHOWN: usize = 26;

/// What NVForever made of one plugin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// No scripts: records only (items, places, NPCs, landscape).
    Data,
    /// Every script runs and names nothing missing.
    Ok,
    /// Runs, with calls or names NVForever does not have.
    Partial,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Data => "OK (data)",
            Status::Ok => "OK",
            Status::Partial => "PARTIAL",
        }
    }
}

/// One plugin's row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModRow {
    pub index: usize,
    pub plugin: String,
    pub status: Status,
    pub scripts: usize,
    /// Units compiled from their source text.
    pub from_source: usize,
    /// Units whose source did not compile (stale), running their compiled bytes.
    pub from_bytes: usize,
    /// Units with extender bytes and no source text to compile.
    pub no_source: usize,
    pub unknown_functions: Vec<String>,
    pub unknown_names: Vec<String>,
}

/// The rows for `load_order`: `units`, every script unit's key (`<plugin>|<owner>`) and whether
/// it holds bytes the decoder could not read; `source`, the source compile's units.
pub fn rows<'a>(load_order: &[String], units: impl Iterator<Item = (&'a str, bool)>, source: &[SourceUnit]) -> Vec<ModRow> {
    let picked: BTreeSet<&str> = source.iter().map(|u| u.key.as_str()).collect();
    let mut totals: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (key, unread) in units {
        let plugin = key.split_once('|').map_or("", |(p, _)| p).to_ascii_lowercase();
        let t = totals.entry(plugin).or_default();
        t.0 += 1;
        t.1 += usize::from(unread && !picked.contains(key));
    }
    load_order
        .iter()
        .enumerate()
        .map(|(index, plugin)| {
            let (scripts, no_source) = totals.get(&plugin.to_ascii_lowercase()).copied().unwrap_or_default();
            let mine: Vec<&SourceUnit> = source.iter().filter(|u| u.plugin.eq_ignore_ascii_case(plugin)).collect();
            let from_bytes = mine.iter().filter(|u| u.error.is_some()).count();
            let once = |names: &dyn Fn(&SourceUnit) -> &Vec<String>| -> Vec<String> {
                let mut seen = BTreeSet::new();
                mine.iter().flat_map(|u| names(u).iter()).filter(|n| seen.insert(n.to_ascii_lowercase())).cloned().collect()
            };
            let unknown_functions = once(&|u| &u.unknown_functions);
            let unknown_names = once(&|u| &u.unknown_enums);
            let status = if scripts == 0 {
                Status::Data
            } else if unknown_functions.is_empty() && unknown_names.is_empty() && no_source == 0 {
                Status::Ok
            } else {
                Status::Partial
            };
            ModRow { index, plugin: plugin.clone(), status, scripts, from_source: mine.len() - from_bytes, from_bytes, no_source, unknown_functions, unknown_names }
        })
        .collect()
}

/// The panel's state.
#[derive(Resource, Default)]
pub struct ModList {
    pub open: bool,
    /// The pause menu's entry asked for it; it opens once the world is back.
    pub open_asked: bool,
    rows: Vec<ModRow>,
    selected: usize,
    first: usize,
}

#[derive(Component)]
struct ModListRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Part {
    Header,
    List,
    Details,
}

pub struct ModListPlugin;

impl Plugin for ModListPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ModList>()
            .add_systems(Update, (toggle, keys, draw).chain().in_set(PlaySet::Ui))
            .add_systems(OnExit(PlayState::Menu), close_on_exit);
    }
}

/// Run condition: the Mod List is open (it holds the keys).
pub fn no_mod_list(m: Option<Res<ModList>>) -> bool {
    !m.is_some_and(|m| m.open)
}

#[allow(clippy::too_many_arguments)]
fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    mut m: ResMut<ModList>,
    state: Res<State<PlayState>>,
    active: Res<crate::menu::ActiveMenu>,
    loading: Res<crate::loading::Loading>,
    script: Option<Res<ScriptState>>,
    mut next: ResMut<NextState<PlayState>>,
    mut commands: Commands,
    roots: Query<Entity, With<ModListRoot>>,
) {
    if !m.open {
        let free = *state.get() == PlayState::Playing && active.0.is_none() && !loading.active();
        if free && m.open_asked {
            m.open_asked = false;
            if m.rows.is_empty() {
                if let Some(data) = script.as_deref().and_then(|s| s.rt.as_ref()).map(|rt| rt.game.data) {
                    let keys: Vec<(String, bool)> = data.units.iter().map(|c| (c.key(), c.raw > 0)).collect();
                    m.rows = rows(&data.load_order, keys.iter().map(|(k, u)| (k.as_str(), *u)), &data.source_units);
                }
            }
            m.open = true;
            NextState::set_if_different(&mut *next, PlayState::Menu);
            spawn(&mut commands);
            info!("mod list: open, {} plugins", m.rows.len());
        }
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        m.open = false;
        next.set(PlayState::Playing);
        for e in &roots {
            commands.entity(e).despawn();
        }
        info!("mod list: close");
    }
}

fn keys(keys: Res<ButtonInput<KeyCode>>, mut m: ResMut<ModList>) {
    if !m.open || m.rows.is_empty() {
        return;
    }
    let n = m.rows.len();
    let mut s = m.selected as i64;
    for (k, by) in [(KeyCode::ArrowDown, 1), (KeyCode::ArrowUp, -1), (KeyCode::PageDown, SHOWN as i64), (KeyCode::PageUp, -(SHOWN as i64))] {
        if keys.just_pressed(k) {
            s += by;
        }
    }
    if keys.just_pressed(KeyCode::Home) {
        s = 0;
    }
    if keys.just_pressed(KeyCode::End) {
        s = n as i64 - 1;
    }
    m.selected = s.clamp(0, n as i64 - 1) as usize;
    let sel = m.selected;
    m.first = crate::menu::model::keep_in_view(m.first, sel, n, SHOWN);
}

fn spawn(commands: &mut Commands) {
    let font = |size: f32| TextFont { font_size: FontSize::Px(size), ..default() };
    let green = Color::srgb(0.75, 1.0, 0.75);
    commands.spawn((
        ModListRoot,
        PointerState::None,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Percent(8.0),
            top: Val::Percent(6.0),
            width: Val::Percent(84.0),
            height: Val::Percent(88.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(14.0)),
            row_gap: Val::Px(10.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.03, 0.0, 0.9)),
        GlobalZIndex(120),
        children![
            (Part::Header, Text::new(""), font(18.0), TextColor(Color::srgb(1.0, 0.95, 0.6))),
            (Part::List, Text::new(""), font(15.0), TextColor(green)),
            (Part::Details, Text::new(""), font(15.0), TextColor(Color::WHITE)),
        ],
    ));
}

/// The header: how many plugins, and how many of each status.
fn header(rows: &[ModRow]) -> String {
    let count = |s: Status| rows.iter().filter(|r| r.status == s).count();
    format!(
        "MOD LIST   {} plugins: {} OK, {} data only, {} partial      Up/Down, Page Up/Down, Home/End move   Esc closes",
        rows.len(),
        count(Status::Ok),
        count(Status::Data),
        count(Status::Partial)
    )
}

fn row_line(r: &ModRow, selected: bool) -> String {
    let mark = if selected { ">" } else { " " };
    let what = if r.scripts == 0 { "no scripts".to_string() } else { format!("{} scripts", r.scripts) };
    format!("{mark} {:02X}  {:<10}  {:<58}  {what}", r.index, r.status.label(), r.plugin)
}

/// The selected plugin's details.
pub fn details(r: &ModRow) -> String {
    let mut out = vec![format!("{}  (load order {:02X})  {}", r.plugin, r.index, r.status.label())];
    if r.scripts == 0 {
        out.push("No scripts: its records (items, places, characters, landscape) load with the game.".into());
    } else {
        out.push(format!(
            "{} scripts: {} run from their source, {} run their compiled bytes (the source is out of date)",
            r.scripts, r.from_source, r.from_bytes
        ));
        if r.no_source > 0 {
            out.push(format!("{} hold extender bytes with no source text: what the decoder cannot read of them does not run", r.no_source));
        }
        if !r.unknown_functions.is_empty() {
            out.push(format!("Calls NVForever does not have yet (they do nothing): {}", r.unknown_functions.join(", ")));
        }
        if !r.unknown_names.is_empty() {
            out.push(format!("Names NVForever has no code for: {}", r.unknown_names.join(", ")));
        }
        if r.status == Status::Ok {
            out.push("Everything its scripts call is built.".into());
        }
    }
    out.join("\n")
}

fn draw(m: Res<ModList>, mut texts: Query<(&Part, &mut Text)>) {
    if !m.open {
        return;
    }
    for (part, mut t) in &mut texts {
        let s = match part {
            Part::Header => header(&m.rows),
            Part::List if m.rows.is_empty() => "The game's scripts are not ready yet: open the Mod List again in a moment.".into(),
            Part::List => m.rows.iter().skip(m.first).take(SHOWN).map(|r| row_line(r, r.index == m.selected)).collect::<Vec<_>>().join("\n"),
            Part::Details => m.rows.get(m.selected).map(details).unwrap_or_default(),
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

/// Something else left the menu state (a travel, a dialogue): the panel goes too.
fn close_on_exit(mut m: ResMut<ModList>, mut commands: Commands, roots: Query<Entity, With<ModListRoot>>) {
    m.open = false;
    for e in &roots {
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fnv_game::source_compile::SourceError;

    fn unit(key: &str, plugin: &str, error: bool, functions: &[&str]) -> SourceUnit {
        SourceUnit {
            key: key.into(),
            plugin: plugin.into(),
            error: error.then(|| SourceError { line: 3, kind: "unresolved_remote_variable", detail: String::new() }),
            unknown_functions: functions.iter().map(|s| s.to_string()).collect(),
            unknown_enums: Vec::new(),
        }
    }

    #[test]
    fn each_plugin_gets_a_status_from_its_scripts() {
        let order: Vec<String> = ["FalloutNV.esm", "Data.esp", "Good.esp", "Stale.esp", "Gap.esp"].map(String::from).to_vec();
        let units = [("Good.esp|SCPT 1", false), ("Stale.esp|SCPT 2", false), ("Stale.esp|SCPT 3", false), ("Gap.esp|SCPT 4", false)];
        let source = [
            unit("Good.esp|SCPT 1", "Good.esp", false, &[]),
            unit("Stale.esp|SCPT 2", "Stale.esp", true, &[]),
            unit("Stale.esp|SCPT 3", "Stale.esp", false, &[]),
            unit("Gap.esp|SCPT 4", "Gap.esp", false, &["NoSuchFn", "nosuchfn"]),
        ];
        let r = rows(&order, units.into_iter(), &source);
        let st: Vec<Status> = r.iter().map(|r| r.status).collect();
        assert_eq!(st, [Status::Data, Status::Data, Status::Ok, Status::Ok, Status::Partial]);
        assert_eq!((r[3].from_source, r[3].from_bytes), (1, 1));
        assert_eq!(r[4].unknown_functions, ["NoSuchFn"]);
        assert!(details(&r[4]).contains("NoSuchFn"));
        assert!(header(&r).contains("5 plugins: 2 OK, 2 data only, 1 partial"));
    }
}
