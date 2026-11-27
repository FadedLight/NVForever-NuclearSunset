# Debug tools: Bug Log and Bug Hunter

Two in-game tools for finding and tracking problems, for people and for AI agents. Both are
local only: no network code, nothing is sent anywhere. Files go to the NVForever home folder
(`Documents\My Games\NVForever`, called `<home>` below).

## Bug Log (F8)

Notes about problems, kept with where you stood when you wrote them.

- **Open**: F8 while playing, or the pause menu's **Bug Log** entry.
- **Keys**: N writes a note (Enter saves, Esc cancels), Up/Down choose a note, F (or Enter)
  marks it fixed or open again, Esc closes.
- **File**: `<home>\bug-log.jsonl`, one JSON object a line:

  | Field | Meaning |
  |---|---|
  | `id` | unix milliseconds, unique |
  | `t` | unix seconds |
  | `note` | your text (up to 500 characters) |
  | `status` | `"open"` or `"fixed"` |
  | `world` | worldspace FormID (number) |
  | `interior` | interior cell FormID, or null outdoors |
  | `pos` | `[x, y, z]` of the player's feet, game units |
  | `shot` | screenshot path or null (not filled yet) |
  | `build` | engine version |

  Another tool may append lines in this shape; a line that is not valid JSON is dropped the
  next time a note is marked fixed.

Source: `engine/nvf-play/src/bug_log.rs`.

## Bug Hunter (console `bughunt`)

Scans the loaded world for collision and clipping problems and writes a report.

### Console

Open the console (the key under Esc) and type:

| Command | What it does |
|---|---|
| `bughunt` | every placed object in the loaded cells |
| `bughunt <radius>` | only within that many game units of the player (e.g. `bughunt 2000`) |
| `bughunt notes [radius]` | round each **open** Bug Log note in the current worldspace or interior (default 768 units); notes elsewhere are counted so you can go there |
| `bughunt show` / `bughunt off` | show or hide the last findings as coloured boxes in the world (shown after every hunt) |

The console prints a one-line summary, the report's path and the first 8 findings.

### What it checks

For every placed reference (REFR) that physics loaded and that is drawn, it compares the box
round its drawn shapes with the box round its collision, and both with what is under them:

| Kind | Box colour | Meaning |
|---|---|---|
| `NoCollision` | magenta | a solid kind of object (STAT, SCOL, FURN, CONT, DOOR, TERM, ACTI) over 48 units with no collision loaded. A suspect: some models ship without collision. Plants, signs, lights, glows, decals, banners, drapes and windows are left out. |
| `Misaligned` | orange | the collision box does not touch the model, or its centre is more than half the model's length away |
| `CollisionTooBig` | red | collision more than 2x (and 64 units) the model on an axis, as with the old Fallout 3 x7 strips bug |
| `Floating` | light blue | a loose item, container or furniture with nothing under it within 6 units; outdoors also a static over 24 units above the terrain with nothing under it |
| `Sunk` | brown | outdoors, a static wholly under the terrain (over 8 units below it everywhere) |
| `FellThrough` | yellow | a dynamic body whose collision is wholly under the terrain |
| `Interpenetrating` | green | a dynamic body overlapping the world by over 3 units in the last physics step |

Objects whose ground is not loaded at all (no terrain, nothing under them) are skipped, not
reported.

### Reports

Each hunt writes two files to `<home>\bug-hunt\`:

- `hunt-<unix seconds>.md`: a table per kind (reference FormID, base type, Editor ID,
  position, detail, model path), for people.
- `hunt-<unix seconds>.json`: the same findings as a JSON array, for tools and AI agents.
  Each item:

  ```json
  {
    "kind": "Floating",
    "form_id": "A403F909",
    "base_type": "MISC",
    "edid": "Wrench01",
    "model": "meshes\\...\\<model>.nif",
    "at": [-75603.0, 4332.0, 9088.0],
    "drawn": { "min": [..], "max": [..] },
    "detail": "327 units over what is under it"
  }
  ```

  `form_id` is the load-order FormID in hex; `at` is the centre of the drawn box in game
  units, usable with `--pos` to go there.

### Headless runs (scripts, CI, AI agents)

`nvf hunt` starts one game at a place, runs `bughunt` and `bughunt notes` from the console
test aid, takes a screenshot with the boxes, and exits:

```
nvf.cmd hunt goodsprings --cell Goodsprings
nvf.cmd hunt megaton --world MegatonWorld --pos -1050,-27560,13300 --heading 270
```

Output: the summary lines from the log, and copies of the reports and screenshot in
`results\bughunt\<label>-*`. The runner is `results\scratch\bug_hunt.ps1`; it uses the
engine's test aids (`NVF_PLAY_TEST_CONSOLE`, `NVF_PLAY_TEST_SHOTS`) and `--run-seconds`,
so any other place arguments the engine takes (`--cell`, `--world`, `--pos`, `--heading`,
`--hour`) work.

A one-line summary is also in the game log for log scrapers:
`bug hunt: <n> references checked in <place> (<ms> ms): <n> findings: NoCollision <n>, ...`

### Tuning

The thresholds are constants at the top of `engine/nvf-play/src/bug_hunt.rs`
(`ITEM_GAP`, `STATIC_GAP`, `SUNK_DEPTH`, `PENETRATION`, `SOLID_MIN_SIZE`, `NOTE_RADIUS`),
and the word lists `is_plant` and `hangs` decide which models are left out. The checks are
plain functions (`check`) over a `Ground` trait, so they are unit tested without a game
(`cargo test -p nvf-play --lib -- bug_hunt`).

Physics side: `fnv_phys::PhysWorld::ref_boxes` (`crates/fnv-phys/src/world.rs`) gives every
placed reference's collision box, dynamic flag and Havok layers.
