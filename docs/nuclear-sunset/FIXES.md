# Fixes that worked — and how they were done

For anyone (person or AI) continuing work on **Nuclear Sunset (FNV + TTW, MO2) on NVForever v0.8.0**.
Each entry: **Problem → Cause → What we did → How to check**. Paths are relative to
`<workspace>\source\NVForever-0.8.0` unless they start with a drive. Full evidence per
engine change: `LOCAL-CHANGES.md` (numbers in brackets). Rules for the work: `AGENTS.md`.

Useful commands used again and again:
- `nvf.cmd build` (engine), `nvf.cmd test integration` / `nvf.cmd test synthetic` (fnv-game tests),
  engine tests: `cargo test --release -p nvforever -- <filter>` / `-p nvf-play --lib -- <filter>` with
  `CARGO_TARGET_DIR=<workspace>\build\target-engine`.
- `fnvtool esm decode --schema <release>\schemas\fnv --edid <EDID> <plugin.esm>` (read a record),
  `fnvtool world refs --world <W> <Data> --mo2-profile <profile folder>` (placed objects),
  `fnvtool vfs mods <instance> --profile <name> --data <Data> --path <file>` (which mod/archive wins),
  `fnvtool nif decode --path <in-bsa path> <archive>` (a mesh), `fnvtool ext report` (scripts).
- Pictures: `results\scratch\cell_shot.ps1` (viewer at a position), play-mode test aids
  `NVF_PLAY_TEST_GOTO/LOOK/MOVE/CONSOLE/SHOTS`, compare with `results\scratch\imgstats.ps1`.
- **After copying a backup file back, touch it** (`(Get-Item f).LastWriteTime = Get-Date`):
  cargo uses timestamps and will not rebuild an old-dated file.

---

## Setup and configuration (no engine code)

**Game started New Vegas's Doc Mitchell intro instead of Nuclear Sunset's start.**
Cause: NVForever doesn't read MO2 profile INIs, where NS sets its start quest.
Did: `Documents\My Games\NVForever\NVForever.ini` → `[General] sCharGenQuest=0x001FFFF8` (TTWStart).
Check: log line `new game: TTWStart started`.

**HUD black after an INI experiment.**
Cause: `[UI] TileMenus=` listing all 18 tile menus (HUD built from tiles).
Did: removed the TileMenus line (default menus). Backups `NVForever.ini.before-tilemenus-*.bak`.
Check: log `script switches: colour 1 130 232 112`; green HUD.

**Game not full screen / launcher didn't start the game.**
Did: `<workspace>\Play Nuclear Sunset (NVForever).cmd` runs our build with
`--data <stock>\Data --mo2 <MO2 instance> --profile "<profile>" --window borderless --upscaler fsr --upscaler-quality quality`,
env `NVF_REPORTS=0`, schemas from the release folder; desktop shortcut to it.

---

## Engine fixes

**[1] Black HUD/Pip-Boy and a start that never began.**
Cause: with `--mo2`, mods' loose config files exist only in the MO2 merge (VFS); NVForever's script
file layer looked only in real folders → MAPMO read its colours as 0 (black UI); Immersive Fast
Travel Encounters couldn't find `Lime/Encounters.ini` and opened an error box hidden behind the black UI.
Did: `crates/fnv-game/src/mod_files.rs` `ModFiles::read`/`read_loose` fall back to the host's
VFS reader; folder listings via new `Roots::list` (`Vfs::loose_paths_under`, set in
`engine/nvf-play/src/script_input.rs`). Do NOT add the 1,665 mod folders to `Roots::read` (it froze
the start). Test `crates/fnv-game/tests/mo2_mod_files.rs`.
Check: log `script switches: colour 1 130 232 112`.

**[2] JIP Companions C&C scripts failed: `RemoveFac` unknown.**
Cause: the GECK accepted `RemoveFac` as RemoveFromFaction (its compiled data holds opcode 0x1180).
Did: alias in `schemas/script/functions.json`. Check: `fnvtool ext report` no longer lists it.

**[3] "Please download the latest version of the yUI" box; game stuck in menu state.**
Cause: NS QuickSelect checks `GetPluginVersion "yUI" < 140`; NVForever reported it absent.
Did: `crates/fnv-script/src/vm/jip.rs` `PLUGINS`: `("yUI", 140.0)` (the owner's original nvse.log
said version 140). Check: no box; score rose 71.6 → 82.5.

**[4] 3–7 FPS and gray textures outdoors at 4K (GPU memory over budget).**
Cause: NS's high-res textures exceed the 8 GB card (7.7 GB of 6 GB budget); paging over PCIe.
Did: texture size cap: `engine/nvf-assets/src/texture.rs` `levels_to_skip`,
`set_max_texture_side` (uploads from the first mip that fits, no resampling);
`engine/nvforever/src/memory_budget.rs`: starts at 2048 on budgets under 10 GB, steps to 1024 above
70 % use; `NVF_TEXTURE_CAP=<side>|off`; now also a setting (see [26]).
Check: log `memory budget` lines; GPU peak ~1.1 GB in the start cave.

**[5] Plugins without FalloutNV.esm as master couldn't see the player/GameHour/Fists.**
Did: `crates/fnv-script/src/compile/env.rs` `def`: forms below 0x800 of the first file resolve for
every plugin. Check: `fnvtool ext report` unknown names 17 → 8.

**[6] `Quest.var` reads of variables the quest doesn't have compiled to a silent 0.**
Did: `crates/fnv-script/src/compile/gen.rs` `names_nothing`: a quest head makes it an unresolved
remote variable → the unit runs its shipped compiled bytes. Check: `ext report`.

**[7] Gray world and red "!" models everywhere; "93 mod archives not mounted".**
Cause: NVForever mounted only `<plugin>.bsa` / `<plugin> - *.bsa`; NS names 67 archives
`NS A Content T 00.bsa` etc.
Did: `crates/fnv-vfs/src/lib.rs` `order_from_owned` claims: an archive named plugin stem + space
belongs to that plugin; start-up warning in `engine/nvforever/src/main.rs` skips them. Test
`archives_named_after_a_plugin_and_a_space_are_its`. Check: viewer status `models … (failed 0)`.

**[8] In-game Mod List.** New `engine/nvf-play/src/mod_list.rs`, pause entry in `pause.rs`
(`Entry::ModList`). Shows each plugin OK / data / PARTIAL and what it lacks.

**[9] Puddles glowing neon green.**
Cause: Tale of Two Landscapes `poolwasteland03.nif` pool shape has emissive (1, .88, 0) ×1 and no
glow map; NVForever added emissive on top in HDR.
Did: `engine/nvf-assets/src/nif_spawn.rs` `shape_emissive_rule` w=1 for lit shapes with emissive
mult ≤ 1, no animated emissive, no External Emittance; `nif_material.wesl` holds emitted light to
the surface colour. Test `ordinary_lit_emitters_are_held_to_their_surface`.
Check: verified on the muck pool only; **not yet checked in the Capital Wasteland**.

**[11] `cherrybomb01.nif` drawn as the red error marker ("footer claims 6291554 roots").**
Cause: the mesh's block-size table is wrong (NiTriStripsData 8050 vs 8782 bytes).
Did: `crates/fnv-nif/src/file.rs` `split_by_decoding`: re-split blocks by decoding when the footer
read is impossible.

**[12] Invisible wall on TTW Megaton's walkway (upstream #3110).**
Cause: bhkNiTriStripsShape vertices are already game units but were scaled ×7 like Havok shapes.
Did: `crates/fnv-collide/src/nif.rs` `strips()`: divide by `HAVOK_SCALE`.
Check: console `tcg 3000` near (−2384, −27990, 13300) in MegatonWorld: the giant triangles are gone.

**[13] NPCs tip over when turning (upstream #2859).** (built, not proven in play)
Did: `engine/nvf-assets/src/anim_play.rs`: a clip named "turn" whose root starts >45° from rest
plays from the rest pose (`TIPPED_ROOT`).

**[14] Too much fog → tweak "Outdoor Fog Distance"** ×1–×4 (`engine/nvf-play/src/tweaks.rs`,
`engine/nvforever/src/sky.rs` `spread`).

**[15] Tweak "No Invisible Walls"**: player ignores collision layer 27 (`tweaks.rs`
`invisible_walls`, `crates/fnv-phys/src/layers.rs` `LayerSet::remove`).

**[16] Ghost image after switching to FSR/DLSS in game (upstream #2955).**
Did: `engine/nvforever/src/taa.rs`: `prepare` removes `TaaView` from views without `TemporalAa`;
`taa_pass` requires `With<TemporalAa>`.

**[17] TTW dialogue shows "SpeechChallengeFailure" as a choice (upstream #3024).**
Did: `crates/fnv-game/src/dialogue.rs` `offer`: never offer that topic through Link From
(`is_speech_failure`).

**[18] No karma sound (upstream #3095).**
Cause: `sim.forms.by_edid("UIKarmaUp")` — SOUN records aren't in that index.
Did: `engine/nvf-play/src/crime.rs`: look up in `Sfx::forms.sounds` by EDID.
Check: `player.rewardkarma 50` → no "karma sound not found" warning.

**[19] Crash from a second controller (upstream #3097, gilrs index out of bounds).**
Did: vendored gilrs 0.11.2 in `engine/vendor/gilrs` (+ `[patch.crates-io]` in `engine/Cargo.toml`);
`next_event_priv` drops events for pads with no data (warning once per pad).

**[20] Grandma Sparkle's Shack (TTW) completely black (upstream #3038).**
Cause: its XCLL fog near 0, far 0, black, no lighting template → everything fully fogged.
Did: `engine/nvforever/src/sky.rs` `interior_fog_far`: far ≤ near means no fog (as outdoors).
Check: `--cell zGrandmaSparkle --pos 2031,2921,8186` shows the room.

**[21] No sound when a script gives caps (upstream #3095).**
Did: `script_events.rs` `ScriptEffect::ItemSound(CAPS)`; `script.rs` `ScriptState::given_sounds`;
`world_sounds.rs` plays the item's pick-up sound. Check: `player.additem f 25` → log
`given by a script, sound ITMBottlecapsUp`.

**[22] "LEVEL UP" shown but the menu never opens (upstream #3030).**
Did: `crates/fnv-game/src/experience.rs` `hunters`: only actors present in loaded cells;
`engine/nvf-play/src/xp_meter.rs`: searchers hold the menu at most `MAX_SEARCH_HOLD` (60 s) after
the fight settles.

**[23] Birch never comes out to the Oasis gate; can't enter Oasis (TTW, upstream #3052).**
Cause: `residents.rs` `take_cells` followed load doors only into interiors; Birch is persistent in
another worldspace (Oasis), so he never got AI.
Did: `engine/nvforever/src/residents.rs` `beyond_door`, `persistent_actors_in`: persistent actors of
the cell a door into another worldspace lands in are handed to the AI once (Low process).
Check: play at `--world Wasteland --pos -10138,117343,17993` for 300 s, `RUST_LOG=info,nvforever::residents=debug`:
`ai: 7 behind a load door into worldspace …`, Birch "arrived", GetDistance to MS10BirchGreetsMarker ≈ 5.

**[24] Weather mod loop: blizzard objects moved every 0.1 s, snow never starts.**
Cause: Desert Natural Weathers calls `GetCellFlag` on a worldspace; NVForever read the WRLD's DATA
(1 = Small World) as "interior", so the mod thought the player was indoors.
Did: `crates/fnv-game/src/nvse_gaps.rs` `getcellflag`: non-CELL forms read 0 (as SetCellFlag).
Check: `RealismBlizzard0?REF moved` appears ~2 times per run instead of thousands.

**[26] More in-game settings**: Graphics → Texture Size Limit (Auto/2048/1024/Off), Distant Tree
Distance; Debug → Show Collision; tweaks placed under proper headings. Files: `memory_budget.rs`,
`draw_distance.rs`, `settings.rs`, `nvf-play/src/collision_view.rs`, `crates/nvf-settings-layout`,
`nvf-play/src/settings_layout.rs`, `docs/settings-reference.md`.

**[27] Area queue support**: `engine/nvforever/src/lod.rs` logs `generated LOD: settled …`; tools
`results\scratch\area_queue.ps1`, `queue_build.py`, `nvf queue markers|grid|run|report`.

**[28] Terrain seam lines (straight lines where ground quadrants meet).**
Cause: the ground shaders multiplied each texture layer's painted opacity by the texture's own
alpha channel. Vanilla landscape textures (DXT1) have no alpha, but Nuclear Sunset's replacements
("NS C NSALVO T 11.bsa", 2048 DXT5) store other data there (RockCanyonRubbleDirt01: alpha ≈ 0.41),
so their layers showed at ~40 % at most. Where one quadrant's base meets the neighbour's layer of
the same texture, one side was full and the other faint → a hard line. It also made all painted
ground (roads, dirt patches) weaker everywhere.
How it was found: `fnvtool world land --full --world WastelandNV --x -18 --y 0` showed the NE
quadrant painting the NW base texture at 0.65–1.0 right up to the edge, yet the picture showed
none of it; our own fnvtool (`build\target-root\release\fnvtool.exe vfs mods … --path
textures\landscape\rockcanyonrubbledirt01.dds`) showed NS's archive wins; extracting it
(`fnvtool bsa extract`) and averaging the DXT5 alpha gave 105/255.
Did: `engine/nvforever/src/terrain.wesl` (both layer loops) and
`engine/nvf-assets/src/nif_surface_bindings.wesl` (detail layers): blend share = painted opacity
only, no `texel.a`. (Layers past the 6th, drawn as separate blended meshes, still use texture
alpha; rare.)
Check: `--cell Goodsprings --pos -71680,3072,9300 --heading 0 --pitch -89 --hour 12`: the line in
`results\visual\seam-before-top.png` is gone in `seam-alpha-top.png`; ground view `seam-alpha.png`.

**[29] Built-in Bug Log (in-game bug tracker, local only).**
What: F8 while playing, or pause menu → **Bug Log**: a panel listing your notes. N writes a note
(Enter saves, Esc cancels), F marks the selected one fixed/open, Up/Down move, Esc closes. Each
note is one JSON line in `Documents\My Games\NVForever\bug-log.jsonl` with the time, worldspace or
interior, position and build. **Never sent anywhere** (no network code; separate from NVForever's
upload-only report form in `engine/nvforever/src/reports/form.rs`, which was not reused).
Did: new `engine/nvf-play/src/bug_log.rs` (modelled on `mod_list.rs`); `pause.rs` `Entry::BugLog`;
`lib.rs` registers `BugLogPlugin`.
Check: play run with `NVF_PLAY_TEST_KEYS=3:f8,5:n` → log `bug log: open, 0 notes`, screenshot
`results\visual\buglog-n.png` shows the panel and the note field. Not yet tried: typing with a real
keyboard (the test keys don't produce text events), a screenshot per note (not built).

**[30] Mod List and Bug Log closed the moment they opened.**
Cause: `engine/nvf-play/src/script.rs` `hold_player` sends `PlayState::Menu` back to `Playing` when
no menu it knows of is open (pause, wait, terminal, lockpick, console, recipe, repair, tables);
our panels weren't in that list, so they were closed the next frame (log: "bug log: open", then
the pointer hidden and the world running on).
Did: `hold_player` returns early while `ModList.open` or `BugLog.open`.
Check: the Bug Log stays up (above). The Mod List should now too (same fix; not yet looked at).

**[32] Pip-Boy off screen: only the glove's fingers at the bottom right when it is opened.**
Cause: Nuclear Sunset's "[#] - Nuclear Sunset Optimization Patch 10" replaces
`meshes\characters\_1stperson\locomotion\male\pipboy.kf` with a 0.067 s clip (`Hit` at 0.033 s,
end 0.067 s; vanilla: `Hit` 0.333, end 0.733). The engine held every Pip-Boy clip at the
vanilla `HOLD_SECONDS` (0.333 s), far past this clip's end, i.e. in its last pose, where the
arm is put away. Nothing to do with resolution or UI scale.
How it was found: `results\scratch\pipboy_shot.ps1` (opens the Pip-Boy with
`NVF_PLAY_TEST_PIPBOY`, shots of STATS/ITEMS/DATA); `fnvtool vfs mods … --path <pipboy.kf>`
said which mod wins; `fnvtool nif decode` of both clips gave their text keys.
Did: `engine/nvforever/src/player_body.rs` `pipboy_clip_at` / `pipboy_time_in`: the Pip-Boy's
own clock (still vanilla seconds, so menus, blur and sounds keep their timing) is mapped onto
the loaded clip: up = clip start..its `Hit` key, down = `Hit`..clip end. Identity for the
vanilla clip; no `Hit` key → the vanilla share of the clip. Used where the clip is posed
(`pose_rig`) and in the hidden warm-up pose. Test `a_short_pip_boy_clip_is_held_at_its_own_hit_key`.
Check: `pipboy_shot.ps1 <exe> <label> 1600x900` and `… borderless`: before
`results\visual\pip-before-900-items.png` (no device), after `pip-after-900-*.png`,
`pip-after-4k-*.png` (device centred, whole).
Note: the raise now takes the vanilla 0.33 s instead of NS's near-instant one.

**[33] Nuclear Sunset's NVR settings carried over (no NVR shader can run here).**
Cause: NVR's shaders are D3D9 programs fed by NVR's DLL (`results\REPORT-shaders-survey.md`).
Did: `engine/nvforever/src/nvr_look.rs` reads `nvse\plugins\NewVegasReloaded.dll.toml` from the
load order; Bloom and Sharpening strengths as ratios to NVR's defaults scale the light glow and
the enhanced Bloom/Sharpening (day/night/interior). Never touches the UI.
Check: log `nvr look: from nvse\plugins\NewVegasReloaded.dll.toml: bloom day x0.40, …`.

**[34] HUD clock/date cut off at the top right.**
Cause: MAPMO's clock centres its text by the text's own `width`; the engine measured widths only
for a few vanilla HUD texts, so mod texts read 0.
Did: `crates/fnv-game/src/ui_hud.rs` `unmeasured_texts` + `engine/nvf-play/src/tiles/draw.rs`
`write_mod_widths`: mod-added HUD text tiles without a file `width` are measured in their font.
Check: `results\scratch\scene_shot.ps1 <exe> x WastelandNV -71492,3650,8470 22 0,180` →
the date reads whole (`results\visual\hudw-night-h0.png`).

**[35] Height fog in place of NVR's Volumetric Fog.**
Did: `engine/nvforever/src/height_fog.rs` + `height_fog.wesl` (engine's own code), strength from
NVR's VolumetricFog Amount via `nvr_look.rs`; world picture only, never sky or UI.
Check: log `nvr look: … height fog x3.00`; `scene_shot.ps1 <exe> x WastelandNV -71492,3650,8470 13 0,180`.

**[38] NVR Cinema (film grain, chromatic aberration, overlay, vignette).**
Did: `engine/nvforever/src/cinema.rs` + `cinema.wesl` (engine's own code), values from NVR's
`[_Shaders.Cinema.Main]` via `nvr_look.rs` `Cinema`; last pass of the main camera (after the
grade and the upscaler), so the HUD never gets grain. Letterbox not taken over.
Check: log `nvr look: … cinema grain 0.30, aberration 1.00, overlay 0.20, vignette 0.00/0.00`;
`scene_shot.ps1 <exe> cine WastelandNV '-71492,3650,8470' 13 '0,180'` (quote the position and
headings when calling from PowerShell, or they arrive space-separated and the game exits 2).

---

## Tried and taken back out (don't repeat blindly)

- **NVR Wet World as a screen-space pass** (normals rebuilt from the depth prepass): stripes and
  speckles with FSR's low-res, jittered depth, even with central differences and only near the
  player (LOCAL-CHANGES 35). Do wetness in the material shaders, which have the real normals.

- **Terrain seams, tries 1–3** (bases half-and-half on edges; all edge vertices agree on one blend
  [25]; each side cross-fades the neighbour's base, saved as `results\scratch\terrain.crossfade-try3.rs`):
  no visible change, because the real cause was the texture alpha (fix [28]).
- **Bright skies (Point Lookout, #2915)**: letting the sky skip the clip before the grade gave
  +1/255 only. Real gap: the weather's "LUM Ramp No Tex" factor is applied in linear space; changing
  that also changes the Mojave.
- **Adding all 1,665 MO2 mod folders to script file roots**: froze the start (1.6 FPS).

## Not bugs (don't "fix")

- NS.esm `aaDCNPCsQuestSCRIPT` "values left on the stack" every 5 s: typo in NS's own script.
- JIP CC&C ">139 plugins" box and NS welcome pages: the modlist's own.
- Rivet City bridge (#2847): the bridge navmesh is disabled in data until NS's bridge script
  enables it; NVForever does this right.
- HUD "HP ☢ 0%" is radiation; crosshair "HP 0/100" is the target's health.
