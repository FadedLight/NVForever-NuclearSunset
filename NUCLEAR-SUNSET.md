# NVForever for Nuclear Sunset (FNV + TTW)

This branch is NVForever upstream `main` as of 2026-11-25 (commit `bd0c8f4`, after tag `v0.8.1`;
first built on `v0.8.0`, `6cb47ad`) with local fixes that make the Nuclear Sunset modlist (Fallout: New Vegas + Tale of Two Wastelands, 251 plugins,
Mod Organizer 2) start and play. The charter in `CLAUDE.md` still holds: clean room, every claim
tagged `[documented]`, `[observed]` or `[hypothesis]`, no game data in the repository.

- Full notes for every change, with its evidence: [`docs/nuclear-sunset/LOCAL-CHANGES.md`](docs/nuclear-sunset/LOCAL-CHANGES.md)
  (numbered 1–38; the numbers below are the same).
- Problem → cause → fix → how to check, plus what was tried and taken back out:
  [`docs/nuclear-sunset/FIXES.md`](docs/nuclear-sunset/FIXES.md).
- Debug tools (Bug Log, `bughunt`): [`docs/debug-tools.md`](docs/debug-tools.md).

## Sync with upstream, 2026-11-25

Upstream `main` (`bd0c8f4`, 2,475 commits past `v0.8.0`) was merged into this branch (a merge
commit, no history rewritten). Upstream had meanwhile fixed several of the issues reported
from here in its own way; where its fix covers ours, **upstream's code was kept and ours
dropped**:

| # | Ours | Now |
|---|---|---|
| 11 | NIF size-table retry in `fnv-nif/src/file.rs` | upstream's same fallback (`size_table_ignored`) |
| 12 | Megaton railing ×7 (#3110) | upstream's identical fix in `fnv-collide/src/nif.rs` |
| 16 | stale TAA view (#2955) | upstream's `drop_stale` in `taa.rs` |
| 17 | SpeechChallengeFailure offered (#3024) | upstream's `player_links` filter in `dialogue.rs` |
| 18, 21 | karma and caps sounds (#3095) | upstream's `sound_by_edid` and `ItemAddedSound` |
| 19 | gilrs second-controller crash (#3097) | upstream's vendored `engine/vendor/gilrs` (same patch, skips to the next event) |
| 20 | interior fog 0..0 (#3038) | upstream's `interior_fog_active` in `sky.rs` |
| 22 | level-up held by searchers (#3030) | upstream's `after_combat::drawn_hunters` in `xp_meter.rs` (our `experience.rs` loaded-cell filter stays) |
| 24 | `GetCellFlag` on a worldspace (#3757) | upstream's CELL-only check in `nvse_gaps.rs` |
| 26 | our "Distant tree distance" slider (percent) | upstream's "Tree distance" row (game units, #3016) |

Kept and merged into upstream's new code: MO2 merged-file listings (`mod_files.rs` `list` next to
upstream's `read_roots`/directory-link walk), yUI 140 next to upstream's Improved Console, the
texture size cap (now also on BC4/BC7, after upstream's `colour_shape_ok`), the terrain
layer-alpha fix (also on the new parallax path), Outdoor Fog Distance (now
`sky.rs` `apply_cell_fog_spread`), No Invisible Walls, the Pip-Boy short-clip fix (inside
upstream's `set_pipboy_layer`), the Mod List/Bug Log hold, `bughunt`, F8 Bug Log and the NVR
look. One lane moved: the emissive hold for ordinary lit emitters (#9) is now
`emissive_rule.y` on lit shapes, because upstream uses `.w` as the authored-material mark.

Checked in the cloud: `cargo check --all-targets` of both workspaces, and the tests of the
root crates this merge touched (fnv-game, fnv-nif, fnv-collide, fnv-vfs, fnv-script,
nvf-settings-layout, fnv-phys), all passing. The engine's own tests were cut short by the cloud
machine's memory; a first run had 6 of about 6,000 failing, two of them fixed since (below).
**Still to do on the PC: the engine tests, clippy, and a game run on the Nuclear Sunset install**
(smoke test, Goodsprings and Megaton bug hunt).

## Release notes

Status words: **verified** = seen working in a game run (log line or screenshot named in the
notes); **built** = compiles and its unit tests pass, not yet seen in a run.

### Collision and the bug hunter

| # | Problem | Cause | Fix (files) | Testing |
|---|---|---|---|---|
| 36 | Parking-lot road (Goodsprings `SCOLParkingLotChunk03bSandy`) had collision 3,922 units long and 1,542 off-centre | a `bhkRigidBodyT` offset was multiplied by its node's scale as well as Havok's ×7; Nuclear Sunset's SCOL meshes have scaled nodes | `crates/fnv-collide/src/nif.rs` `to_object`; `docs/spec-notes/collision.md` | fnv-collide tests pass; bug hunt Goodsprings misaligned/too-big 1 → 0, Megaton floating 4 → 0 (**verified**) |
| 37 | Bug hunter said items on the gas-station roof floated 325 units up | its ray started 1 unit over the item's bottom, under a roof the item is set into | `engine/nvf-play/src/bug_hunt.rs` `rest_gap` | `bug_hunt` tests 8 pass; Goodsprings floating 13 → 4, the roof items clear (**verified**) |
| 31 | No tool to find collision and clipping problems | — | console `bughunt` (`engine/nvf-play/src/bug_hunt.rs`, `console.rs`, `crates/fnv-phys/src/world.rs` `ref_boxes`), `docs/debug-tools.md` | **verified** in Goodsprings and Megaton |
| 12 | Invisible wall across TTW Megaton's walkway (upstream #3110) | `bhkNiTriStripsShape` vertices are game units but were scaled ×7 | `crates/fnv-collide/src/nif.rs` `strips` | collision view before/after (**verified** visually) |
| 15 | Option to walk through invisible walls | — | tweak "No Invisible Walls" (`tweaks.rs`, `fnv-phys/src/layers.rs`) | built |

### Starting the modlist, scripts and UI

| # | Problem | Cause | Fix (files) | Testing |
|---|---|---|---|---|
| 39 | New game took 98 s to the world, then ~2.5 min more for the new-game scripts (after the upstream sync) | upstream's Script Runner files ran FileExists, INI and folder reads that walked all 1,665 MO2 mod folders on disk | `fnv-game/src/mod_files.rs` `Roots::hosted_from`/`disk_roots`, `fnv-vfs` sorted loose listing, `nvf-play/src/script_input.rs` | crate tests; new game 6.2 s to the world (**verified**, log named in note 39) |
| 40 | "Outdated xNVSE plugins detected!" box | Engine Optimizations / HighResLocalMaps reported absent | `fnv-script/src/vm/jip.rs` `PLUGINS` | box gone (**verified**) |
| 41 | "B42 Inertia missing requirement" box | its check ran before the player's first-person model existed | `nvf-play/src/script.rs`, `fnv-game/src/nif_blocks.rs` | box gone after new game and load (**verified**) |
| 42 | "Please enable JohnnyGuitar patches: JIP Fixes" box | JohnnyGuitar.ini read from stock Data, not the MO2 merge | `fnv-game/src/nvse_jg7.rs` | box gone (**verified**) |
| 43 | "Problem compiling ScaledRecurringChallenges.gek" box | IsFormValid said no to CompileScript's result | `fnv-game/src/game.rs` `form_sig` | box gone (**verified**) |
| 44 | TTW "falloutcustom.ini misconfigured" box | the MO2 profile's FalloutCustom.ini was not read | `fnv-install/src/mo2.rs` `PROFILE_INIS` | box gone (**verified**) |
| 1 | Black HUD/Pip-Boy; start-up error box | mod INI/config files exist only inside the MO2 merge | `crates/fnv-game/src/mod_files.rs`, `crates/fnv-vfs`, `nvf-play/src/script_input.rs` | test `mo2_mod_files.rs`; log `script switches: colour 1 130 232 112` (**verified**) |
| 2 | JIP Companions C&C scripts failed | `RemoveFac` is an accepted alias of RemoveFromFaction | `schemas/script/functions.json` | `fnvtool ext report` |
| 3 | "Download the latest yUI" box, game stuck in menu state | yUI reported absent | `crates/fnv-script/src/vm/jip.rs` | **verified** (no box) |
| 5, 6 | Plugins without FalloutNV.esm as master; `Quest.var` reads compiled to 0 | form resolution / stale source | `fnv-script/src/compile/env.rs`, `gen.rs` | `ext report` unknown names 17 → 8 |
| 7 | Gray world, red "!" models; 67 archives not mounted | archives named `<plugin> <anything>.bsa` were not claimed | `crates/fnv-vfs/src/lib.rs`, `engine/nvforever/src/main.rs` | fnv-vfs tests; viewer `models … (failed 0)` (**verified**) |
| 8, 29, 30 | In-game Mod List and Bug Log (F8); panels closed at once | — / `hold_player` closed unknown menus | `mod_list.rs`, `bug_log.rs`, `pause.rs`, `script.rs` | Bug Log **verified**; Mod List built |
| 11 | `cherrybomb01.nif` drawn as the error marker | wrong block-size table in the mesh | `crates/fnv-nif/src/file.rs` `split_by_decoding` | `fnvtool nif decode` reads all blocks |
| 17 | TTW dialogue offered "SpeechChallengeFailure" (upstream #3024) | Link From made it a choice | `crates/fnv-game/src/dialogue.rs` | integration tests; not seen in a run |
| 18, 21 | No karma / caps sounds (upstream #3095) | SOUN not in the EDID index; script-given items had no sound | `nvf-play/src/crime.rs`, `script_events.rs`, `script.rs`, `world_sounds.rs` | log lines (**verified**) |
| 22 | "LEVEL UP" but no menu (upstream #3030) | searchers in unloaded cells held it forever | `crates/fnv-game/src/experience.rs`, `nvf-play/src/xp_meter.rs` | tests; not seen in a run |
| 23 | TTW Birch never comes to the Oasis gate (upstream #3052) | persistent actors behind a load door into another worldspace got no AI | `engine/nvforever/src/residents.rs` | **verified** (Birch arrived, 300 s run) |
| 24 | Weather mod loop, snow never starts | `GetCellFlag` on a worldspace read WRLD data | `crates/fnv-game/src/nvse_gaps.rs` | **verified** (2 moves instead of thousands) |
| 32 | Pip-Boy off screen | NS's short `pipboy.kf` held past its end | `engine/nvforever/src/player_body.rs` | screenshots 1600×900 and 4K (**verified**) |
| 34 | HUD clock/date cut off | mod HUD texts had width 0 | `crates/fnv-game/src/ui_hud.rs`, `nvf-play/src/tiles/draw.rs` | screenshot (**verified**) |

### Rendering, performance and settings

| # | Problem | Cause | Fix (files) | Testing |
|---|---|---|---|---|
| 4 | 3–7 FPS, gray textures at 4K on 8 GB | NS textures over the GPU budget | texture size cap (`nvf-assets/src/texture.rs`, `nvforever/src/memory_budget.rs`) | GPU peak ~1.1 GB in the start cave (**verified**) |
| 9 | Puddles glowing neon | emissive added on top in HDR | `nvf-assets/src/nif_spawn.rs`, `nif_material.wesl` | **verified** on one pool |
| 13 | NPCs tip over when turning (upstream #2859) | tipped root in turn clips | `nvf-assets/src/anim_play.rs` | built, not proven |
| 14, 26 | Too much fog; more settings | — | tweaks and settings rows (`tweaks.rs`, `sky.rs`, `settings*.rs`, `nvf-settings-layout`, `docs/settings-reference.md`) | tests pass |
| 16 | Ghost image after switching to FSR/DLSS (upstream #2955) | TAA view kept | `nvforever/src/taa.rs` | built |
| 19 | Crash with a second controller (upstream #3097) | gilrs index out of bounds | vendored `engine/vendor/gilrs` + `[patch.crates-io]` | built |
| 20 | Grandma Sparkle's Shack black (upstream #3038) | fog range 0..0 meant full fog | `nvforever/src/sky.rs` | screenshot (**verified**) |
| 27 | Area queue support | — | `nvforever/src/lod.rs` log line | used by the queue tools |
| 28 | Terrain seam lines | landscape layers multiplied by texture alpha (NS DXT5 textures carry other data there) | `nvforever/src/terrain.wesl`, `nvf-assets/src/nif_surface_bindings.wesl` | top-down screenshots (**verified**) |
| 33, 35, 38 | Nuclear Sunset's NVR look (bloom, sharpening, volumetric fog, Cinema) | NVR's D3D9 shaders cannot run here | own code reading NVR's settings: `nvr_look.rs`, `height_fog.rs/.wesl`, `cinema.rs/.wesl`, `graphics.rs`, `post.rs` | log lines and screenshots; see notes 33/35/38 |

## Building

```
cd engine
cargo build --release -p nvforever
```

Run with the modlist: `nvforever --data "<Stock New Vegas>\Data" --mo2 "<MO2 instance>" --profile "<profile>"`
(plus the release's `--esm-schema`/`--nif-schema` folders). NVForever never writes into the
game folder or `Documents\My Games\FalloutNV`.
