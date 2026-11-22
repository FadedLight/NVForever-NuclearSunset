# NVForever for Nuclear Sunset (FNV + TTW)

This branch is NVForever **v0.8.0** (upstream tag `v0.8.0`, commit `6cb47ad`) with local fixes
that make the Nuclear Sunset modlist (Fallout: New Vegas + Tale of Two Wastelands, 251 plugins,
Mod Organizer 2) start and play. The charter in `CLAUDE.md` still holds: clean room, every claim
tagged `[documented]`, `[observed]` or `[hypothesis]`, no game data in the repository.

- Full notes for every change, with its evidence: [`docs/nuclear-sunset/LOCAL-CHANGES.md`](docs/nuclear-sunset/LOCAL-CHANGES.md)
  (numbered 1–38; the numbers below are the same).
- Problem → cause → fix → how to check, plus what was tried and taken back out:
  [`docs/nuclear-sunset/FIXES.md`](docs/nuclear-sunset/FIXES.md).
- Debug tools (Bug Log, `bughunt`): [`docs/debug-tools.md`](docs/debug-tools.md).

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
