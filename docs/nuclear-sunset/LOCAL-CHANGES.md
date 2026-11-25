# Local changes to NVForever v0.8.0 (Nuclear Sunset stability pass)

Each change: what, why, the evidence (tagged as the project's charter asks), and how it was checked.
Written from the game data, the run logs and NVForever's own notes only (no nv-rs, no extender source).

1. **Mod INI files through an MO2 merge** (`crates/fnv-game/src/mod_files.rs`, `ModFiles::read`, `read_loose`).
   With `--mo2`, a mod's loose `Config` files are in the merge the host reads, not in any folder on disk, so
   `GetINIFloat_Cached` and the other INI readers found nothing. Now they fall back to the host's reader.
   [observed: Nuclear Sunset's Immersive Fast Travel Encounters reads `Lime/Encounters.ini`, which is in its
   mod's `config\Lime`; NVForever answered 0 and the mod showed "Error001 ... Check that Encounters.ini file
   is present in Data > Config" at a new game's start.]

   Folder listings too: a new `Roots::list` (the host's loose files under a folder) set from
   `Vfs::loose_paths_under` (new, `crates/fnv-vfs`) in `engine/nvf-play/src/script_input.rs`
   `mod_file_roots`; `files_under`, `files_in` and `entries_in` add those after the on-disk roots
   (kNVSE animation folders, GetFilesInFolder). [observed: MAPMO's colours from
   `Config\Main And Pause Menus Overhaul - Presets.ini` read as 0, logged `script switches: colour 1 0 0 0`,
   and the whole HUD and Pip-Boy drew black.] Test: `crates/fnv-game/tests/mo2_mod_files.rs`.
   Tried and reverted: adding the 1,665 MO2 mod folders to `Roots::read`: every lookup walked them on
   disk and a new game never left the loading screen (1.6 FPS).

36. **A bhkRigidBodyT's offset is not scaled by its node** (`crates/fnv-collide/src/nif.rs`
    `RawCollision::to_object`; test `conversion_scales_and_applies_body_transform_only_for_t`;
    `docs/spec-notes/collision.md` placement chain). The body translation takes the Havok
    scale (x7) only; the node's scale still scales the shape (the ragdoll already did this).
    [observed 2026-11-22: NS "NS C NSALVO M 00.bsa" `meshes\scol\scolparkinglotchunk03bsandy.nif`
    (`results\scratch\scol_dump.py`): 8 bhkRigidBodyT on nodes scaled 0.91-1.99; unscaled,
    each body's translation cancels its packed vertices' centre to within 1 unit (Havok5:
    572.68 x 7 = 4009 against 309.6 x 7 x 1.85 = 4009); scaled, Havok5's collision sat 3,400
    units from its road piece: bug hunt "collision 3922 units on x against the model's 825"
    and "collision centre 1542 units from the model's", Goodsprings 00106B76.] Tests:
    fnv-collide 29 passed. Verified: `nvf hunt` Goodsprings (`results\bughunt\gs6-*`):
    Misaligned 1 -> 0, CollisionTooBig 1 -> 0; Megaton (`megaton3-*`): Floating 4 -> 0
    (HoldingTank05 and RocketFort no longer over nothing). Not yet walked over in play.

37. **Bug hunter's floating check probes from inside the model** (`engine/nvf-play/src/bug_hunt.rs`
    `rest_gap`, `REST_PROBE`). The ray down starts 16-32 units over the model's bottom (its
    own reference excluded) and the gap is counted from the bottom, so an item set a few
    units into a roof or shelf is held. [observed 2026-11-22: the Goodsprings gas station
    (SCOLgsGasStation 0017748C, `scolgsgasstation.nif` Havok2) has roof collision 46.64 Havok
    units (326) over its base (`results\scratch\tri_under.py`), where the mod-placed roof vent,
    toolbox and wrench (A403F908-A) stand; the old ray started 1 unit over their bottoms, under
    the roof, and reported them "325 units over the terrain".] Test
    `loose_items_float_unless_something_is_under_them` extended. Run gs6 (probe at the model's
    height): vent and toolbox no longer reported, Floating 13 -> 5; the 3-unit wrench still
    was, hence the 16-unit minimum. Run gs7 (`results\bughunt\gs7-*`): the wrench is
    clear too; Floating 4 left (three wood-plank junk piles 39-77 units up, a metal box 7
    up: placements in the data, not checked further).

38. **NVR Cinema look, own code** (new `engine/nvforever/src/cinema.rs`, `cinema.wesl`;
    `main.rs` `mod cinema`; `graphics.rs` registers `CinemaPlugin`; `nvr_look.rs` `NvrLook::cinema`,
    struct `Cinema`). Reads `[_Shaders.Cinema.Main]` when `[_Shaders.Cinema.Status] Enabled`:
    FilmGrainAmount, ChromaticAberration, OverlayStrength, VignetteDarkness/Radius, each held to a
    range. One full-size pass, the last of the main camera's post-processing (after the
    image-space grade, so after FSR too): red/blue split growing with the square of the distance
    from the centre (none at the reticle), an overlay blend of the picture on itself, a vignette,
    then mean-zero grain at 24 grain frames a second, weighted to the mid-tones (no lifted blacks
    at night, no speckled glare). The letterbox (AspectRatio, Mode, LetterBoxDepth) is not taken
    over: Hikikomori's Mode 3 is outside the 0-2 NVR's defaults file lists, and black bars over
    play are not wanted. [observed: Hikikomori NVR Cinema on, grain 0.3, aberration 1.0, overlay
    0.2, vignette 0/0; meanings from the comments in NVR's `NewVegasReloaded.dll.defaults.toml`;
    scales by eye.] Log: `nvr look: … cinema grain 0.30, aberration 1.00, overlay 0.20, vignette
    0.00/0.00`. Tests: nvforever `nvr_look` 3, `cinema` 1, `height_fog` 1 passed. Runs
    (`results\visual\cine-{day,night}-h{0,180}.png` vs `cineoff-…` with `NVF_NVR_LOOK=off`, FSR
    on, 1600x900): HUD glyph pixels at night differ ≤0.18/255 (by day 1-3.5/255, from glyph edges
    blended over a different world behind them, since the off run also drops bloom and fog);
    the lit windows glow the same in both; day sky high-frequency detail 0.9 → 2.9 (the grain),
    night grain near nothing by design. `NVF_NVR_LOOK=off` turns this off with the rest.
    The off run turns the whole NVR look off, so the shots do not isolate Cinema alone.

35. **Height fog (NVR Volumetric Fog's look, own code)** (new `engine/nvforever/src/height_fog.rs`,
    `height_fog.wesl`; `graphics.rs` registers `HeightFogPlugin` and puts `HeightFog` on the main
    camera; `nvr_look.rs` `NvrLook::fog`). A full-size pass after TAA/AO, before sun shafts,
    bloom and the image-space blur: exponential height haze (e-folding 1,500 units, reference
    300 under the eye) in the weather's fog colour, optical depth 0.12 × NVR Amount ratio over
    the weather's fog far distance, a sun lobe by day; outdoors only, never on sky pixels,
    nothing without NVR settings (ratio 0). [design: owner 2026-11-21 "convert the shaders from
    nuclear sunset"; observed: Hikikomori VolumetricFog on, Amount 1.553 vs NVR default 0.4 →
    held to ×3.] Tests: `height_fog` 1, `nvr_look` 2 passed. Runs: midday
    `results\visual\fog-day-h*.png` (far hills hazy, HUD intact); night `fog-night-h180.png` vs
    `hudw-night-h180.png`: far band −0.4/255, sky 0.02, HUD 0.19 mean diff; rain
    (`--weather NVDLC02ZVRainWeather`, `rainfog-h180.png`): clean, no banding. Strength
    constants by eye only; not yet seen in dust weathers or the Capital Wasteland.
    `NVF_NVR_LOOK=off` turns the whole NVR look (33, 35) off for comparing.
    Tried and taken back out the same day: NVR's Wet World as part of this pass (surfaces facing
    up, from normals rebuilt out of the depth prepass, darkened to NVR's puddle tint with a
    sheen). With FSR the depth is 1066x600 and jittered, so the rebuilt normals flipped from row
    to row: horizontal stripes over the hills, then over near ground and speckles round the
    cactus even with central differences and a 5,000-unit reach (`results\visual\wet-h180.png`,
    `wet2-`, `wet3-` vs `wetoff-h180.png`). Wetness needs the real normals, in the materials.

34. **Mod-added HUD texts get their measured width** (`crates/fnv-game/src/ui_hud.rs`
    `unmeasured_texts`, `MEASURED_MARK`; `engine/nvf-play/src/tiles/draw.rs`
    `FontKit::write_mod_widths`, `TileFonts::mod_widths`). Every `text` under HUDMainMenu that
    the engine did not already measure and whose file gives no `width` is measured in its own
    font (only when its string or wrap changes; notices keep their own path). [observed
    2026-11-21: MAPMO's `hudmainmenu\clock.xml` places its Time text at (dial width − text
    width) / 2; the width read 0, so ", 17.08.2277, 2…" ran off the right edge
    (`results\visual\gs-night-h0.png`).] Verified: `results\visual\hudw-night-h*.png` show
    ", 17.08.2277, 22:07" whole; the rest of the HUD pixel-identical to before (mean diff ≤ 0.04
    at heading 0). Tests: nvf-play `tiles::` 520 passed. The leading ", " (an empty first part
    of MAPMO's string) is not explained yet.

33. **NVR look converter** (new `engine/nvforever/src/nvr_look.rs`; `graphics.rs` registers
    `NvrLookPlugin`; `post.rs` `image_space_glow` now `pub(crate)`). Reads the load order's
    `nvse\plugins\NewVegasReloaded.dll.toml` (Nuclear Sunset: "Hikikomori NVR") and takes Bloom
    Strength (Main/Night/Interiors) and Sharpening Strength as ratios to NVR's shipped defaults
    (1.0, 0.75), held to 0.25–3; applies them to the light glow and the enhanced Bloom /
    Sharpening, day and night blended by the sun's height. World passes only, before the UI.
    [observed: Hikikomori bloom 0.4 / 5.0 / 2.3, sharpening 1.0 on; log line `nvr look: …
    bloom day x0.40, night x3.00, interiors x2.30; sharpening x1.33`.] Tests: `nvr_look` 2
    passed. Night run: saloon windows' area +0.6/255 mean luma, HUD unchanged
    (`results\visual\nvr-night-h180.png` vs `nvr2-night-h180.png`). With the owner's settings
    (light glow on, enhanced Bloom off, FSR on) only the light-glow part acts.

32. **The Pip-Boy clip is held at its own `Hit` key** (`engine/nvforever/src/player_body.rs`
    `pipboy_clip_at`, `pipboy_time_in`; used in `pose_rig`'s Pip-Boy layer and the warm-up pose).
    The Pip-Boy's clock stays in the original clip's seconds; the time put on the clip maps
    0..0.333 onto start..`Hit` and 0.333..0.733 onto `Hit`..end of the clip actually loaded.
    [observed 2026-11-21: Nuclear Sunset Optimization Patch 10's loose `pipboy.kf` is 0.067 s,
    `Hit` 0.033 (`fnvtool nif decode`); held at 0.333 it showed its last, arm-down pose and the
    owner saw the Pip-Boy off screen; vanilla's from `Fallout - Meshes.bsa`: `Hit` 0.333, end 0.733.]
    Test `a_short_pip_boy_clip_is_held_at_its_own_hit_key`; nvforever `pip_boy` tests 4 passed.
    Verified: `results\scratch\pipboy_shot.ps1` at 1600x900 and borderless 3840x2160
    (`results\visual\pip-before-900-items.png` vs `pip-after-900-*.png`, `pip-after-4k-*.png`).

31. **Bug Hunter: console `bughunt`** (new `engine/nvf-play/src/bug_hunt.rs`; `console.rs` command,
    help line and `hunt_lines`; `lib.rs` plugin; `bug_log.rs` `path`/`load` now `pub(crate)`;
    `crates/fnv-phys/src/world.rs` new `PhysWorld::ref_boxes` and `RefBox`, re-exported in `lib.rs`;
    `nvf.ps1` `hunt`; runner `results\scratch\bug_hunt.ps1`; guide `docs/debug-tools.md`).
    Scans the loaded cells (or a radius, or round open Bug Log notes) for missing, misplaced or
    oversized collision, floating and buried models, bodies under the terrain and overlapping
    dynamic bodies; report `.md` + `.json` in `<home>\bug-hunt\`, boxes drawn in the world.
    [design: owner 2026-11-21, "a bug hunter tool ... check for Collison issues and texture
    clipping".] Tests: nvf-play `bug_hunt` 8 passed. Runs 2026-11-21 in Goodsprings
    (`nvf hunt goodsprings3 --cell Goodsprings`, log nvforever-20261121-121753-848.log): 1,513
    references in 19 ms; 292 findings at first, 27 after leaving out plants, hanging signs/lights/
    decals, roads set flush and bases set into the ground (those were the bulk, checked by hand
    in the JSON). Left to look at: SCOLParkingLotChunk03bSandy (00106B76) collision 3,922 units
    long against an 825-unit model and 1,542 units off-centre; a roof vent, wrench and toolbox
    (A403F908-A) 325 units up with nothing under them, next to it.

30. **Mod List and Bug Log stay open** (`engine/nvf-play/src/script.rs` `hold_player`): it returns
    early while either panel is open. [observed 2026-11-21: "bug log: open" then straight back to
    Playing.] Verified for the Bug Log (`results\visual\buglog-n.png`); Mod List not looked at.

29. **Built-in Bug Log** (new `engine/nvf-play/src/bug_log.rs`, `pause.rs` `Entry::BugLog`, `lib.rs`).
    F8 / pause entry; local JSON lines in `<NVForever home>\bug-log.jsonl`; no network code.
    [design: owner 2026-11-21 "I'd rather that be built in".] Verified opening and the note field;
    typing with a real keyboard not yet tried. Tests not run (owner: save usage).

28. **Terrain layers blend by their painted opacity alone** (`engine/nvforever/src/terrain.wesl`
    both layer loops; `engine/nvf-assets/src/nif_surface_bindings.wesl` detail layers). The layer
    share was `opacity * texel.a`; Nuclear Sunset's DXT5 landscape textures carry other data in alpha
    (RockCanyonRubbleDirt01 from "NS C NSALVO T 11.bsa": mean 105/255), so layers showed at ~40 % and
    the Goodsprings NW/NE quadrant edge drew a hard seam (NE paints the NW base at 0.65–1.0 at the edge
    per `fnvtool world land --full`). [observed 2026-11-21; hypothesis: the original does not use a
    landscape diffuse's alpha as a blend mask, vanilla's DXT1 has none.] Verified: top-down shot
    `results\visual\seam-alpha-top.png` has no line (before: `seam-before-top.png`); 64 % of pixels
    changed as painted ground now shows at full strength. Tests not run (owner: save usage).
    A third seam try (base cross-fade in `stitch_layers`) was set aside unbuilt-in, saved as
    `results\scratch\terrain.crossfade-try3.rs`.

27. **"Generation settled" log line** (`engine/nvforever/src/lod.rs` `poll_generated`, `GenState::busy`):
    `generated LOD: settled, N chunks ready (B baked this run, C from the cache), S skipped` each
    time the bakes around the camera are all in, for the area queue
    (`results\scratch\area_queue.ps1`, `nvf queue`). [design: owner 2026-11-20.] Verified: the
    queue trial (Megaton walkway, Arefu) read it.

26. **New in-game settings** (agent draft applied; `memory_budget.rs` Texture Size Limit
    Auto/2048/1024/Off, `draw_distance.rs` Distant Tree Distance, `nvf-play/collision_view.rs` Debug
    Show Collision, `nvf-settings-layout` placements incl. Outdoor Fog Distance and No Invisible
    Walls, `nvf-play/settings_layout.rs`, `docs/settings-reference.md`). [design: owner 2026-11-20
    "more settings in-game".] Tests: nvforever 21 passed, nvf-play 28 passed, settings layout 4
    passed. Not clicked through in game yet.

25. **(Tried and reverted) quadrant edges agree on one blend** (`terrain.rs` `agree_edges`). The
    visible Goodsprings seam is a hard line between two different base textures, which this patch
    leaves alone by design: before/after top-down shots identical in mean colour
    (`results\visual\seam-before-top.png`, `seam-after-top.png`). Reverted; seam still open.

24. **`GetCellFlag` answers 0 for anything but a cell** (`crates/fnv-game/src/nvse_gaps.rs`).
    [observed: Desert Natural Weathers calls it on worldspace TestMap01 at load; WRLD DATA 1 (Small
    World) read as the interior flag, so the mod thought the player was indoors and re-parked
    RealismBlizzard01/02REF 5,252 times in 5 minutes (log nvforever-20261121-015227-3128.log), and
    its outdoor snow could never start.] Test: `jip8` `cells_and_water` extended; integration 996
    passed. Verified: a 60 s Goodsprings run (nvforever-20261121-022740-10292.log) moved them twice.

23. **Actors behind a load door into another worldspace get AI** (`engine/nvforever/src/residents.rs`
    `beyond_door`, `persistent_actors_in`, `take_cells`). The persistent actors of the cell a door
    into another worldspace lands in are handed to the AI once, at the Low level. [observed: upstream
    issue #3052, TTW's Birch never came out to the Oasis gate.] Test
    `a_load_door_into_another_worldspace_hands_over_the_persistent_actors_beyond_it` passed. Verified
    (log nvforever-20261121-015227-3128.log): "ai: 7 behind a load door into worldspace 060A0C2B",
    Birch used OasisDoorToWasteland, arrived after 111 s, started MS10BirchGreetsPlayerAtGates;
    GetDistance to his marker 5.49.
    Also tried and reverted the same day: a bright-sky clip skip (#2915, agent patch): the Point
    Lookout sky band changed by 1/255 only. Rivet City (#2847): no engine bug (the bridge navmesh is
    disabled in the data until NS's bridge script enables it).

22. **A level-up is no longer kept back for good by searchers** (`crates/fnv-game/src/experience.rs`
    `hunters`; `engine/nvf-play/src/xp_meter.rs` `level_up`, `MAX_SEARCH_HOLD`). Only actors present
    in a loaded cell count as hunting the player; and once the fight has settled, actors still
    searching hold the level-up menu at most 60 s (HUD up, out of menus), then the level is taken
    with a log line. [observed: upstream issue #3030 (open), "level up waits: 3 actors fighting or
    searching" after "the fight is over", then no level-up, "XP 466/200"; design for the 60 s.]
    Test `a_level_is_earned_by_experience_and_waits_for_no_one_hunting_the_player` extended (an
    actor whose cell was let go does not hunt); synthetic 162 passed, integration 996 passed. Not
    seen in a run.

21. **Script-given caps play their pick-up sound** (`engine/nvf-play/src/script_events.rs`
    `ScriptEffect::ItemSound`; `script.rs` `ScriptState::given_sounds`; `world_sounds.rs`
    `objects`). Caps added to the player with the "added" line shown play the caps' own pick-up
    sound once per batch, as caps taken from a container. [observed: upstream issue #3095 (the other
    half of 18).] Run 2026-11-20 (`results\scratch\caps_test.ps1`, log
    nvforever-20261120-163409-9824.log): `player.additem f 25` logged "Bottle Cap (25) added" and
    "world sounds: 0x0000000F given by a script, sound ITMBottlecapsUp"; the silent
    `player.additem f 5 1` played nothing. Unit test in `script_events.rs` (caps: sound once, then the
    line; silent caps: neither): nvf-play `script_events` tests 15 passed.

20. **An interior whose fog range is unset has no fog** (`engine/nvforever/src/sky.rs`
    `interior_fog_far`). Fog far not beyond near (XCLL fog 0..0) now means no fog indoors, as a
    weather's unset range already did outdoors. [observed: upstream issue #3038 (open), TTW's
    Grandma Sparkle's Shack (zGrandmaSparkle) drew entirely black; its XCLL has fog near 0, far 0,
    colour black, no lighting template (`fnvtool esm decode`). hypothesis: the original shows such a
    room unfogged.] Test `an_unset_interior_fog_range_is_no_fog`. Run 2026-11-20: the shack was all
    black on the release and on the previous build (`results\visual\sparkle-*.png`), and draws its
    kitchen with this build (`results\visual\sparkle-fixed2.png`).

19. **A controller event for an unknown pad no longer panics** (`engine/vendor/gilrs`, new vendored
    copy of gilrs 0.11.2, `[patch.crates-io]` in `engine/Cargo.toml`, `src/gamepad.rs`
    `next_event_priv`; `engine/vendor/README.md`). An event for a pad id with no data is dropped
    with one warning per pad. [observed: upstream issue #3097 (open), crash "index out of bounds:
    the len is 1 but the index is 1" at gilrs gamepad.rs:474, a second controller's input.]
    Engine builds; no controller test (none here).

18. **Karma changes play their sounds** (`engine/nvf-play/src/crime.rs` `show_notes`). `UIKarmaUp` /
    `UIKarmaDown` are looked up in the sound effects' SOUN list (`Sfx::forms.sounds`, as
    `xp_meter::play_sound`); the form store's Editor ID index has no SOUN records, so the old lookup
    never found them. A missing one logs `crime: karma sound … not found`. [observed: upstream issue
    #3095 (open).] Run 2026-11-20 (`results\scratch\karma_test.ps1`, log
    nvforever-20261120-155142-3308.log): `player.rewardkarma 50` and `-50` logged both karma changes
    and no "not found" warning. (Sound plays are not logged; not listened to.) The script-given caps
    sound half of #3095 is not done.

17. **TTW: "SpeechChallengeFailure" is no longer offered as a dialogue choice**
    (`crates/fnv-game/src/dialogue.rs` `offer`, `is_speech_failure`). The topic is what an actor says
    when a speech challenge fails; its lines' Link From made it a topic-list entry labelled with its
    own name. [observed: upstream issue #3024 (open), Megaton's Lucas Simms; Fallout3.esm has 74 such
    lines.] Test `the_speech_failure_topic_is_known_by_name`; fnv-game integration tests: 996 passed.
    Not seen in a run (needs Megaton and MQ01 progress).

16. **TAA stops when an upscaler takes over in game** (`engine/nvforever/src/taa.rs` `prepare`,
    `taa_pass`). A view whose `TemporalAa` was removed drops its `TaaView`; the pass needs
    `TemporalAa`. [observed: upstream issue #2955 (open), a faint frozen picture over the view after
    switching to FSR/DLSS in the pause menu; the reporter tested this fix.] Builds; a run with FSR from
    the start draws clean (`results\visual\karma-fixed.png`). Switching in game not tested here.

15. **Tweak "No Invisible Walls"** (Settings > Tweaks; `engine/nvf-play/src/tweaks.rs` `invisible_walls`,
    `crates/fnv-phys/src/layers.rs` `LayerSet::remove`). Off by default. On: the player's character ignores
    collision layer 27 (Invisible Wall); shots, sight and the crosshair already passed it. Re-applied
    each frame (the character is rebuilt on travel). [design: owner 2026-11-20.] Not seen in a run yet.

14. **Tweak "Outdoor Fog Distance"** (Settings > Tweaks; `tweaks.rs` `outdoor_fog_distance`,
    `engine/nvforever/src/sky.rs`). x1.0 (the weather's own, default) to x4.0: outdoor fog near and far
    times it; the `--fog` test override is left alone. [design: owner 2026-11-20 "there is just too much
    fog".] Not seen in a run yet.

13. **NPCs no longer tip over in turn clips** (`engine/nvf-assets/src/anim_play.rs` sampling, `TIPPED_ROOT`).
    A turn clip ("turn" in its sequence name) whose accumulation root starts with its up axis over 45°
    from the skeleton's rest plays from the rest instead, keeping only its rotation relative to its start
    (r' = rest x start^-1 x r). [observed: upstream issue #2859 (open; draft PR #2860 not released),
    `2haturnleft/right.kf` hold Bip01 almost horizontal and carry `Bip01 NonAccum`, so `root_turn_out`
    kept the start; owner 2026-11-20: "npcs tip over when they go to move ... fall over then stand back
    up".] Getting-up and knock-down clips are left alone (name check). Runs 2026-11-20
    (`results\scratch\turn_test.ps1`, `--actor-weapons drawn`, a Goodsprings settler and Primm's
    NCR trooper close up): upright in both builds, but both held pistols, so the two-handed turn
    clips were not played. Still unproven.

12. **Invisible walls from Fallout 3 collision** (`crates/fnv-collide/src/nif.rs` `strips`).
    bhkNiTriStripsShape's NiTriStripsData vertices are game units; they were scaled x7 like Havok shapes.
    [observed: upstream issue #3110 (open; draft PR #3111 not released): TTW Megaton's
    MegatonRampTurn90Sml railing became a wall across the walkway, 19 FO3 shape trees use it.] fnv-collide
    tests pass (29). Run 2026-11-20 (`results\scratch\collision_view.ps1`, console `tcg`, from
    (-1050, -27560, 13300) heading 270): the release draws long extra collision triangles across the
    walkway that this build does not (`results\collision\zoom-release.png` / `zoom-fixed.png`). Not
    yet walked through in play.

11. **A NIF whose block-size table is wrong is split by decoding** (`crates/fnv-nif/src/file.rs`,
    `split_by_decoding`). When the footer read after the size table is impossible, the blocks are split
    by decoding them with the schema (the path files without a table already take) and the footer read
    after them. [observed 2026-11-20: NS's "NS C NSALVO M 00.bsa" `meshes\clutter\junk\cherrybomb01.nif`
    stores NiTriStripsData as 8050 bytes, decoding reads 8782; "footer claims 6291554 roots" and the error
    marker; with the fallback `fnvtool nif decode` reads all 10 blocks ok, roots [0].] Check script:
    `results\scratch\nif_header.py`.

10. **(Tried and reverted) terrain bases half-and-half on quadrant edges.** It did not remove the seam
    and `terrain.rs` is upstream's again (checked identical to the source zip). See HANDOFF "Terrain
    quadrant seams" for the real cause.

9. **Ordinary lit emitters are held to their surface colour** (`engine/nvf-assets/src/nif_spawn.rs`
    `shape_emissive_rule` w; `nif_material.wesl`). A lit shape with emissive multiplier <= 1, no material
    clip and no External Emittance: its emitted light only fills the pixel up to its texture colour.
    [observed 2026-11-20: Tale of Two Landscapes' `poolwasteland03.nif` (loose in NS's "Sneezer Stuff")
    pool surface, emissive (1.0, 0.88, 0.0) x 1, no glow map, drew neon yellow-green] [hypothesis: the
    original's lighting sum with emissive is held to 1 before it multiplies the texture]. Neon over 1
    unchanged. Test: `ordinary_lit_emitters_are_held_to_their_surface`.

8. **In-game Mod List** (`engine/nvf-play/src/mod_list.rs`, new; entry "Mod List" in the pause
   menu column, `pause.rs`; test action `NVF_PLAY_TEST_PAUSE_MENU=<s>:mod-list`). Every plugin of the
   load order with OK / OK (data) / PARTIAL and, for the chosen one, its scripts (from source, from
   compiled bytes), the functions and names NVForever lacks. Static facts only (the source compile's
   results), not runtime failures yet. The engine builds with it; its unit test and the pause menu's
   label tests were updated but **not yet run**, and the panel has not been seen in a run.

7. **Archives named "<plugin> <anything>.bsa" belong to that plugin** (`crates/fnv-vfs/src/lib.rs`
   `order_from_owned` `claims`; the start-up warning in `engine/nvforever/src/main.rs` no longer lists
   them). NVForever mounted only `<stem>.bsa`, `<stem> - *.bsa` and `<stem>N…`, so **67 of Nuclear
   Sunset's archives were never mounted**: "NS A Content T 00.bsa" … "NS Z LOD Terrain T 01.bsa", its
   packed textures, meshes, sounds, voices, faces, weapons, creatures, power armour, UI and LOD.
   [observed 2026-11-20: start-up said "93 mod archive(s) are not ticked … so not mounted"; the profile's
   archives.txt is empty and no INI lists them; the modlist plays on the original game; the owner saw
   gray, untextured surfaces and a glowing green puddle.] [hypothesis: the original takes the plugin
   name as a prefix.] A glued name (`MergeExtra.bsa`) is still not claimed (#2521's rule kept).
   Test: fnv-vfs `archives_named_after_a_plugin_and_a_space_are_its`; all fnv-vfs tests pass.
   Script `results\scratch\bsa_prefix.py` lists which archives each rule claims (67 added, 5 left are
   the base game's, loaded by the INI list).

6. **`Quest.name` with no such quest variable is stale source, not a call**
   (`crates/fnv-script/src/compile/gen.rs` `names_nothing`). The compiler took `Quest.name` (name not
   in the quest's script) as an unknown function called on the quest, so the script compiled from source
   with that read returning 0. Now a quest head makes it an unresolved remote variable, so the unit runs
   its shipped compiled bytes, as for EDGE TTW already. [observed: STAR TTW 2.esp reads
   `QSTStartupSTARS.fVisionRemainingSeconds`; its quest script SCPTStartupSTARS declares
   `fVisionRemainingPerc`.] Effect on NS (`fnvtool ext report`): Dynamic World, Healing Systems,
   STAR TTW 2, Long 15 Reloaded now run their bytes for one unit each; still 3,586 units picked.

5. **The game's own forms are seen by every plugin** (`crates/fnv-script/src/compile/env.rs` `def`).
   FormIDs below 0x800 of the first file (the player 0x14, GameHour 0x38, Fists 0x1F4) resolve even
   when the compiling plugin does not list FalloutNV.esm as a master. [observed: Helmet Overlay.esm has
   no masters; its compiled SCPT 0x816 lists SCRO 0x14 and 0x38 for `float fHour = GameHour`.]
   Effect: GameHour (Helmet Overlay) and Fists (Weapon Requirement System) no longer unknown.
   NS's unknown names: 17 -> 8. `cargo test -p fnv-script`: all pass.

4. **Texture size cap when GPU memory is short** (`engine/nvf-assets/src/texture.rs`
   `levels_to_skip`, `set_max_texture_side`; `engine/nvforever/src/memory_budget.rs`).
   A DDS bigger than the cap that carries its smaller mip levels is uploaded from the first level that
   fits (no resampling). The cap starts at 2048 on a GPU budget under 10 GB, steps to 1024 when this
   process holds over 70% of the budget (before the caches start dropping textures at 85%), and
   `NVF_TEXTURE_CAP=<side>|off` overrides it. [observed 2026-11-20: at 4K outdoors the game held 7673 MB
   of a 6085 MB budget, every GPU pass slowed tenfold (3-7 FPS) and the owner saw gray, untextured
   surfaces; with a 1024 cap from the start the run held at most 3844 of 7239 MB and the cave rendered
   correctly (`results\shot-latest.png`).] Tests: nvf-assets `a_size_cap_leaves_out_the_largest_levels`,
   `a_capped_texture_uploads_from_the_first_level_that_fits`; nvforever `texture_cap_tests`.
   Still to verify: the Capital Wasteland outdoors with the automatic cap (needs the owner to pick the
   start in the choice box).

3. **yUI reported as installed, version 140** (`crates/fnv-script/src/vm/jip.rs` `PLUGINS`).
   [observed: the owner's original-game nvse.log, "yUI.dll (yUI; version 140; infoVersion 1) loaded
   correctly"; NS QuickSelect.esp's source: `if GetPluginVersion "yUI" < 140` → MessageBoxEx "Please
   download the latest version of the yUI" → `return`, so Quick Select never set itself up and a new
   game sat in menu state behind the box.] Its other yUI paths also need tile `HUDMainMenu/ySI/_ySI`
   = 1, which no built menu sets, so they stay off. The only other NS plugin asking is MAPMO (prints
   the version). Tests: `synthetic.rs` `built_in_plugins_report_installed`,
   `plugin_questions_are_kept_only_when_asked_for` (now use NVTF/ITR as the absent plugin).

2. **`RemoveFac` alias of RemoveFromFaction** (`schemas/script/functions.json`).
   [observed: JIP Companions Command & Control.esp's script source calls `rAux.RemoveFac JIPCCCIsHired`, and
   its compiled data (SCDA) holds RemoveFromFaction's opcode 0x1180 at those calls, so the GECK took the
   name as RemoveFromFaction.] `fnvtool ext report` listed it as unknown in 3 plugins (4 units).

39. **Start-up 98 s → 6 s: file questions through the MO2 index** (after the upstream sync of
    2026-11-25). Upstream's Script Runner now runs Nuclear Sunset's 219 `gr_*.txt` files when the
    game starts and its `ln_*.txt` files at a new game or load, behind the loading screen. Upstream's
    read roots also hold every MO2 mod folder (`vfs.loose_roots()`, 1,665 of them), and each
    FileExists, INI read, CompileScript read and folder listing walked them all on disk.
    [observed: log `nvforever-20261125-145542-7604.log`: restart files `fileexists 149x 19811 ms,
    getinisection 482x 11657 ms, compilescript 600x 9365 ms`; new-game files `fileexists 505x
    76944 ms`; loading screen down after 79.98 s, then 314.14 s; `gr_Keybinder.txt took 24433 ms`.]
    Fix: `Roots::hosted_from` (`crates/fnv-game/src/mod_files.rs`) marks the read roots the host's
    index already holds; `Roots::disk_roots` keeps only NVForever's layer and Data on disk, and
    `exists`, `read_data_file`, `read_loose`, `ModFiles::read`, `data_files`, `entries_in`,
    `list_files`, `read_data_folder_inis`, FileExists and GetFilesInFolder/GetFoldersInFolder ask
    the index (`lookup`, `reader`, `list`) for the rest. Set in `engine/nvf-play/src/script_input.rs`
    `init_mod_file_roots`. `crates/fnv-vfs` `loose_paths_under` keeps the loose keys sorted once and
    lists a folder as a range of them (was a pass over every key, ~200 ms a listing). Timing aids
    kept in the log: `script runner: <file> took N ms` for files over 250 ms and each moment's
    slowest extender functions (`script_runner.rs`, `game.rs` `ext_call`).
    Checked: crate tests fnv-vfs, fnv-script, fnv-game (lib, mo2_mod_files, script_runner,
    get_files_data_roots, synthetic, small_libs, jg9) pass, new test
    `hosted_roots_tests`; new game on this PC (Ryzen 7 5800XT held at base clock), log
    `nvforever-20261125-152502-11852.log`: `loading screen down after 6.21 s`, restart files'
    slowest function 239 ms in all, new-game files `fileexists 505x 607 ms`.

40. **Engine Optimizations 100 and HighResLocalMaps 12 reported** (`crates/fnv-script/src/vm/jip.rs`
    `PLUGINS`). A Script Runner check showed "Outdated xNVSE plugins detected! ... Engine
    Optimizations: -1 -> 100+ / HighResLocalMaps: -1 -> 12+" at a new game [observed: log
    `nvforever-20261125-142835-8120.log`]. Their work is the engine's own here; the versions are the
    owner's [observed: nvse.log in the stock game folder, "Engine Optimizations; version 100" and
    "HighResLocalMaps; version 12"], as yUI's 140 (change 3). Checked: the box is gone in
    `nvforever-20261125-152502-11852.log`.

41. **Script Runner new-game and load files wait for the player's models**
    (`engine/nvf-play/src/script.rs` `new_game_files`/`load_files`, `NEW_GAME_FILES_WAIT`;
    `crates/fnv-game/src/nif_blocks.rs` `SceneMirror::watch`/`has_tree`). B42 Inertia's
    `ln_B42Inertia.txt` asks `Player.GetNifScale "NeckController" 2` a frame after it runs;
    the player's first-person tree had not been told yet (nor watched), the answer was 0 and
    "B42 Inertia missing requirement! Install latest Compatibility Skeleton" showed [observed:
    logs `nvforever-20261125-152502-11852.log` (new game) and `-155050-8284.log` (load); the
    console asked the same in a loaded game and read 1, `-153358-7876.log`]. The player is
    watched from the start, and the `ln_`/`gl_`/`gn_` files run once the first-person tree is
    there (at most 300 frames). Checked: no B42 box after a new game (`-154352-15152.log`) or a
    load (`-155959-14680.log`).

42. **JohnnyGuitar.ini and nvse_config.ini through the merged Data**
    (`crates/fnv-game/src/nvse_jg7.rs` `data_text`). GetJohnnyPatch read
    `Data\NVSE\Plugins\JohnnyGuitar.ini` from the stock Data only; with `--mo2` it sits in a mod
    folder (`bJIPFixes = 1`), so patch 8 read 0 and MCM Extender showed "Please enable the
    following JohnnyGuitar patches: > JIP Fixes: 0 -> 1" [observed: `-154352-15152.log`].
    Checked: `GetJohnnyPatch 8` = 1 at the console (`-155050-8284.log`), box gone.

43. **A CompileScript result is a valid form** (`crates/fnv-game/src/game.rs` host
    `form_sig`: compiled scripts and lambdas answer SCPT). Scaled Recurring Challenges checks
    `IsFormValid` on CompileScript's result and showed "There was a problem compiling
    ScaledRecurringChallenges.gek" although it compiled (the console's CompileScript gave a
    form, `-155050-8284.log`). Also logged now: why a CompileScript file did not compile or
    was not found (`compile script: ...`). Checked: box gone (`-155959-14680.log`).

44. **The MO2 profile's FalloutCustom.ini is read** (`crates/fnv-install/src/mo2.rs`
    `PROFILE_INIS`, after Fallout.ini and FalloutPrefs.ini). TTW's start check
    `GetNumericIniSetting "bBackgroundLoadLipFiles:BackgroundLoad" == 0` showed
    TTWBadINIMessage: the profile's Fallout.ini has 0, its FalloutCustom.ini 1 [observed: TTW's
    script source in TaleOfTwoWastelands.esm; the profile's files]. Checked: fnv-install tests
    pass; box gone after a load (`-160651-12920.log`). Left: NS's welcome pages, the 139-plugin
    warning and Functional Post Game Ending's "overwritten by" notice, which the original shows
    with this load order too [hypothesis for the last: a load-order conflict, owner's call].

45. **DDS files with the alpha-pixels bit next to their FourCC are read**
    (`crates/fnv-dds/src/lib.rs` `PixelFormat::format`). The reader matched the pixel
    format's flags as a whole, so flags 0x5 (FourCC + alpha pixels) with `DXT5`/`DXT1` were
    "unsupported pixel format" and the shapes drew white [observed: Honest Hearts'
    `textures\nvdlc02\landscape\trees\nvdlc02treebark.dds`, `nvdlc02treebrnc.dds`,
    `nvdlc02treeleaf2zv.dds`, `nvdlc02_redcedarneedlesev_lod.dds` (white trees in Zion) and
    Nuclear Sunset's `loading_desktop06.dds`; log nvforever-20261125-185028-13396.log, 7
    textures failed]. Checked: new test `a_fourcc_with_the_alpha_pixels_bit_reads_as_its_fourcc`,
    fnv-dds 29 pass; Zion from above again: 0 "unsupported pixel format", textures failed 7
    → 1, the trees textured (`results\visual\dlc\fnv-honest-hearts-zion.png` vs `-before.png`).

46. **Ground seams with "Terrain anti-tiling" on: change 28 put back on that path**
    (`engine/nvforever/src/terrain.wesl`, the `TERRAIN_ANTI_TILING` block). Upstream's
    anti-tiling path, new in the 2026-11-25 merge, weighed each layer by painted opacity ×
    the diffuse's alpha, which change 28 had removed from the other paths; with anti-tiling
    on (which turns the textures, the owner's "turned texture") Nuclear Sunset's DXT5
    landscape textures left a hard straight line where one quadrant's base meets the
    neighbour's layer (the owner's report, 2026-11-25). Found by checking every line our first
    pass added against the merged code. Now the painted opacity alone. Checked:
    `--cell Goodsprings --pos -71680,3072,9300 --pitch -89 --hour 12 --graphics-only
    terrain-anti-tiling`, `results\visual\at2-gs-before.png` (a vertical line down the middle)
    against `at2-gs-after.png` (blended; 51 % of sampled pixels changed).

47. **Change 36 put back** (`crates/fnv-collide/src/nif.rs` `RawCollision::to_object`):
    the bhkRigidBodyT translation takes HAVOK_SCALE only, not the node's scale; the merge had
    kept upstream's `v * f`. Bug hunt Goodsprings again showed 00106B76
    (SCOLParkingLotChunk03bSandy) Misaligned 1542 and CollisionTooBig 3922. Checked: fnv-collide
    tests (the bhkRigidBodyT expectation back to 7 / -7) pass; bug hunt Goodsprings
    Misaligned 1 → 0, CollisionTooBig 1 → 0.

48. **NVR Water Lens, own code** (`engine/nvforever/src/cinema.rs`, `cinema.wesl`,
    `nvr_look.rs` `NvrLook::water_lens`, struct `WaterLens`). Reads `[_Shaders.WaterLens.Main]`
    when `[_Shaders.WaterLens.Status] Enabled` (Hikikomori keeps NVR's defaults: Amount 0.1,
    Time 180, TimeMultA 0.1, TimeMultB 0.2, Viscosity 0.05) [observed: the profile's
    NewVegasReloaded.dll.toml]. When the camera comes up out of the water (upstream's
    `underwater::CameraSubmerged`), drops on the lens bend the picture behind them, run down
    and fade over Time read as frames at 60 a second [hypothesis: 180 = 3 s]; their rims are
    lifted 4 % and the Cinema colour fringes follow the bent read. Drawn in the Cinema pass,
    also when NVR's Cinema is off. Own code from the setting names and the effect's look, not
    from NVR's shader. Checked: cinema tests (7) pass incl. `water_drops_fade_after_surfacing`;
    play run at Lake Mead (WastelandNV cell 28,16), dived to 1,800 and surfaced at 2 s / 5 s:
    log "water lens: out of the water, drops on the lens"; `results\visual\lens-surfaced.png`
    shows the drops, `lens-gone.png` (4 s later) none.
