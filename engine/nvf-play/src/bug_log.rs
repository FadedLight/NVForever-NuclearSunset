//! The Bug Log: the owner's own, local-only bug notes, kept in the game [design: owner
//! 2026-11-21, "I'd rather that be built in"]. The pause menu's "Bug Log" entry (or F8 while
//! playing) opens a panel over the world: N writes a note about a problem, F marks the selected
//! note fixed or open again, Up/Down move, Esc closes.
//!
//! **Never sent anywhere.** This module has no network code and does not touch `reports`
//! (nvforever's player-report form); it only appends to `<home>\bug-log.jsonl` (home =
//! `Documents\My Games\NVForever`), through `fnv_install::guard_write`. One JSON object a line:
//! `id` (unix ms, unique), `t` (unix s), `note`, `status` ("open"/"fixed"), `world` and
//! `interior` (FormIDs as numbers), `pos` (game-space feet), `shot` (screenshot path or null),
//! `build`. Marking fixed rewrites the file whole; a line that is not valid JSON is dropped then.

use crate::player::Player;
use crate::ui_pointer::PointerState;
use crate::{Location, PlaySet, PlayState};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

const SHOWN: usize = 14;
const MAX_NOTE: usize = 500;
const FILE: &str = "bug-log.jsonl";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Note {
    pub id: u64,
    pub t: u64,
    pub note: String,
    pub status: String,
    pub world: u32,
    pub interior: Option<u32>,
    pub pos: [f32; 3],
    #[serde(default)]
    pub shot: Option<String>,
    #[serde(default)]
    pub build: String,
}

pub(crate) fn path() -> Option<PathBuf> {
    fnv_install::EngineHome::user().map(|h| h.config_file().with_file_name(FILE))
}

pub(crate) fn load(p: &std::path::Path) -> Vec<Note> {
    std::fs::read_to_string(p).map(|t| t.lines().filter_map(|l| serde_json::from_str(l).ok()).collect()).unwrap_or_default()
}

fn append(p: &std::path::Path, n: &Note) -> Result<(), String> {
    fnv_install::guard_write(p, None).map_err(|e| e.to_string())?;
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p).map_err(|e| e.to_string())?;
    writeln!(f, "{}", serde_json::to_string(n).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn rewrite(p: &std::path::Path, notes: &[Note]) -> Result<(), String> {
    fnv_install::guard_write(p, None).map_err(|e| e.to_string())?;
    let body: String = notes.iter().filter_map(|n| serde_json::to_string(n).ok()).map(|l| l + "\n").collect();
    let tmp = p.with_extension("jsonl.tmp");
    std::fs::write(&tmp, body).and_then(|()| std::fs::rename(&tmp, p)).map_err(|e| e.to_string())
}

#[derive(Resource, Default)]
pub struct BugLog {
    pub open: bool,
    /// The pause menu's entry asked for it; it opens once the world is back.
    pub open_asked: bool,
    notes: Vec<Note>,
    /// Row in the list, newest first.
    selected: usize,
    typing: bool,
    draft: String,
    status: String,
}

#[derive(Component)]
struct BugLogRoot;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Part {
    Header,
    List,
    Footer,
}

pub struct BugLogPlugin;

impl Plugin for BugLogPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BugLog>()
            .add_systems(Update, (update, draw).chain().in_set(PlaySet::Ui))
            .add_systems(OnExit(PlayState::Menu), close_on_exit);
    }
}

#[allow(clippy::too_many_arguments)]
fn update(
    keys: Res<ButtonInput<KeyCode>>,
    mut typed: MessageReader<KeyboardInput>,
    mut b: ResMut<BugLog>,
    state: Res<State<PlayState>>,
    active: Res<crate::menu::ActiveMenu>,
    loading: Res<crate::loading::Loading>,
    loc: Option<Res<Location>>,
    player: Option<Res<Player>>,
    mut next: ResMut<NextState<PlayState>>,
    mut commands: Commands,
    roots: Query<Entity, With<BugLogRoot>>,
) {
    let events: Vec<KeyboardInput> = typed.read().filter(|k| k.state.is_pressed()).cloned().collect();
    let free = *state.get() == PlayState::Playing && active.0.is_none() && !loading.active();
    if !b.open {
        if free && keys.just_pressed(KeyCode::F8) {
            b.open_asked = true;
        }
        if free && std::mem::take(&mut b.open_asked) {
            b.notes = path().map(|p| load(&p)).unwrap_or_default();
            (b.open, b.selected, b.typing, b.status) = (true, 0, false, String::new());
            NextState::set_if_different(&mut *next, PlayState::Menu);
            spawn(&mut commands);
            info!("bug log: open, {} notes", b.notes.len());
        }
        return;
    }
    let n = b.notes.len();
    if b.typing {
        for k in events {
            match &k.logical_key {
                Key::Escape => b.typing = false,
                Key::Backspace => {
                    b.draft.pop();
                }
                Key::Enter => {
                    if b.draft.trim().is_empty() {
                        b.status = "Write something first.".into();
                        continue;
                    }
                    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                    let note = Note {
                        id: now.as_millis() as u64,
                        t: now.as_secs(),
                        note: b.draft.trim().to_string(),
                        status: "open".into(),
                        world: loc.as_ref().map_or(0, |l| l.world),
                        interior: loc.as_ref().and_then(|l| l.interior),
                        pos: player.as_ref().map_or([0.0; 3], |p| p.feet),
                        shot: None, // TODO: reuse the report form's screenshot (see notes)
                        build: env!("CARGO_PKG_VERSION").into(),
                    };
                    b.status = match path().ok_or_else(|| "no home folder".to_string()).and_then(|p| append(&p, &note)) {
                        Ok(()) => "Saved.".into(),
                        Err(e) => format!("Could not save: {e}"),
                    };
                    info!("bug log: note saved ({} characters)", note.note.chars().count());
                    b.notes.push(note);
                    (b.typing, b.selected, b.draft) = (false, 0, String::new());
                }
                _ => {
                    if let Some(t) = k.text.as_deref().filter(|t| t.chars().all(|c| !c.is_control())) {
                        for c in t.chars() {
                            if b.draft.chars().count() < MAX_NOTE {
                                b.draft.push(c);
                            }
                        }
                    }
                }
            }
        }
        return;
    }
    if keys.just_pressed(KeyCode::Escape) {
        b.open = false;
        next.set(PlayState::Playing);
        for e in &roots {
            commands.entity(e).despawn();
        }
        info!("bug log: close");
    } else if keys.just_pressed(KeyCode::KeyN) {
        (b.typing, b.draft, b.status) = (true, String::new(), String::new());
    } else if n > 0 {
        let mut s = b.selected as i64 + i64::from(keys.just_pressed(KeyCode::ArrowDown)) - i64::from(keys.just_pressed(KeyCode::ArrowUp));
        s = s.clamp(0, n as i64 - 1);
        b.selected = s as usize;
        if keys.any_just_pressed([KeyCode::KeyF, KeyCode::Enter]) {
            let i = n - 1 - b.selected;
            let fixed = b.notes[i].status == "open";
            b.notes[i].status = if fixed { "fixed" } else { "open" }.into();
            b.status = match path().ok_or_else(|| "no home folder".to_string()).and_then(|p| rewrite(&p, &b.notes)) {
                Ok(()) => String::new(),
                Err(e) => format!("Could not save: {e}"),
            };
        }
    }
}

fn spawn(commands: &mut Commands) {
    let font = |size: f32| TextFont { font_size: FontSize::Px(size), ..default() };
    commands.spawn((
        BugLogRoot,
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
            (Part::List, Text::new(""), font(15.0), TextColor(Color::srgb(0.75, 1.0, 0.75))),
            (Part::Footer, Text::new(""), font(15.0), TextColor(Color::WHITE)),
        ],
    ));
}

fn draw(b: Res<BugLog>, mut texts: Query<(&Part, &mut Text)>) {
    if !b.open {
        return;
    }
    let open = b.notes.iter().filter(|n| n.status == "open").count();
    for (part, mut t) in &mut texts {
        let s = match part {
            Part::Header => format!("BUG LOG   {} notes, {open} open   (saved on this PC only)   N new, F fixed/open, Up/Down, Esc closes", b.notes.len()),
            Part::List if b.notes.is_empty() => "No notes yet. Press N to write one.".into(),
            Part::List => {
                let first = crate::menu::model::keep_in_view(0, b.selected, b.notes.len(), SHOWN);
                b.notes
                    .iter()
                    .rev()
                    .enumerate()
                    .skip(first)
                    .take(SHOWN)
                    .map(|(row, n)| {
                        let mark = if row == b.selected { ">" } else { " " };
                        let first_line = n.note.lines().next().unwrap_or("").chars().take(90).collect::<String>();
                        format!("{mark} {:<6} {:08X}{} {first_line}", n.status.to_uppercase(), n.interior.unwrap_or(n.world), if n.interior.is_some() { " int" } else { "    " })
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Part::Footer if b.typing => format!("New note (Enter saves, Esc cancels):\n{}_", b.draft),
            Part::Footer => b.status.clone(),
        };
        if t.0 != s {
            t.0 = s;
        }
    }
}

fn close_on_exit(mut b: ResMut<BugLog>, mut commands: Commands, roots: Query<Entity, With<BugLogRoot>>) {
    b.open = false;
    for e in &roots {
        commands.entity(e).despawn();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_round_trip_as_json_lines_and_bad_lines_are_skipped() {
        let dir = std::env::temp_dir().join(format!("nvf-buglog-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(FILE);
        let n = Note { id: 1, t: 2, note: "door sticks".into(), status: "open".into(), world: 0xDA726, interior: None, pos: [1.0, 2.0, 3.0], shot: None, build: "x".into() };
        std::fs::write(&p, "not json\n").unwrap();
        // Write directly (guard_write only allows the engine home).
        std::fs::write(&p, format!("not json\n{}\n", serde_json::to_string(&n).unwrap())).unwrap();
        assert_eq!(load(&p), vec![n]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
