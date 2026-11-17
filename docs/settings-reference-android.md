# Android settings reference supplement

These additional rows and compatibility notes belong to the Android-integrated
engine. The shared desktop reference remains [settings-reference.md](settings-reference.md);
the desktop launcher's four editable tabs and its built-in table are unchanged.
This document does not add Mobile settings editing to the launcher. Android rows
keep their existing in-game placements in desktop builds; selecting a physical
input device does not change the Android build's Mobile grouping.

## Mobile › Touch layout and guide

Android groups these actions under Mobile; desktop keeps them in Controls › Touch.

### `touch-layout-editor-v1`: Customize layout

Opens the existing control editor over the paused Settings page. Move or resize
controls and change opacity, aiming or profiles. Save uses `touch-controls.json`;
Cancel or Escape restores the original draft. Both return to Settings. The action
does not persist an open session and refuses another active form or a window
smaller than 400 × 300 logical units.

### `touch-practice-v1`: Controls help / gesture practice

Opens isolated movement, aiming and gesture lessons without sending gameplay
actions. Close or Escape returns to Settings. Its existing saved key is unchanged.
These actions replace the removed persistent settings/help shortcuts on the HUD.

## Mobile optional interaction preferences

These rows use the normal `[Settings]` INI consumer. Enhanced views, hints,
appearance and the existing text-size/confirmation choices are independent.
Android uses the Mobile headings below; the desktop settings layout retains
Gameplay > Dialogue/Inventory menus and Controls > Touch.

| Mobile heading | Key | Behavior |
|---|---|---|
| Inventory and quick access | `touch-terminal-v1` | Enhanced terminal view; Off retains the standard menu/input path. |
| Inventory and quick access | `touch-companion-v1` | Enhanced companion wheel; independent of drag-release preference. |
| Inventory and quick access | `touch-pipboy-dismiss-side-v2` | Downward drags outside the display: Disabled (default), Left only, Right only or Both. Legacy side switches still load and save; the chooser replaces their visible rows. |
| Inventory and quick access | `touch-pipboy-dismiss-travel-v1` | Arm-lowering drag distance, 48–240 logical units, default 120; capped by remaining safe space. Reverse while held to raise it again. |
| Inventory and quick access | `touch-pipboy-close-button` | Optional explicit Close target; off by default. |
| Inventory and quick access | `touch-pipboy-button-graphics` | Show overlay paint; Off keeps the same active targets and dispatch. DefaultOn. |
| Inventory and quick access | `touch-pipboy-model-wheel` | Touch the physical model wheel to scroll; on by default, independent of mouse/pad scrolling. |
| Movement and gestures | `touch-vats-targeting-v1` | Off (default), Tap or Hold; body selection queues, separate Fire confirms. |
| Dialogue | `dialogue-theme-v1` | Minimal (default), Classic Glass, Field Notes, Retro Console or Wrist Terminal. The older Split Panel value still loads. |
| Dialogue | `dialogue-background-tint-v1` | HUD colour (default), Neutral or Warm amber backing; text and selection retain the chosen HUD colour. |
| Dialogue | `dialogue-background-opacity-v1` | Backing opacity, default 18%, maximum 80%; the stored slider is normalized. |
| Dialogue | `dialogue-frame-opacity-v1` | Accent opacity, default 60%; independent of backing and full-contrast text/feedback. |
| Dialogue | `dialogue-hints-v1` | Show explanatory dialogue hints, off by default; choices, speech and requirements stay. |
| Lockpicking | `lockpick-touch-hints` | Show lockpicking hints, on by default; scheme, Turn, status and errors stay. |

Appearance updates existing UI; it does not change choice words/order, semantic
actions or input bindings. New controls are authored but uncompiled/unrun until
separately qualified; their registration is not device acceptance.

## Input preferences alongside Tweaks

[design] The explicit custom left/right stick-deadzone preference supplies its
per-stick values only while enabled. Otherwise the separately saved generic
controller calibration applies. Neither option rewrites the other preference.
Keyboard/mouse with a selected controller remains opt-in: actual physical holds
stay independent of controller aim/sneak toggles and of virtual touch input.
Movement and look device transforms are applied before the independently
configured gyroscope contribution. Device/cover/focus cancellation also clears
deferred reload input and stale held Jump state.

These are the integrated source contracts and authored control boundaries;
current-head compiler/runtime and real Bluetooth-controller acceptance are pending.

## Display › Interface

- Compatibility row: the main list and its saved names remain registered; the wider `hud-scale-v2` row represents it in the combined settings page.

### `hud-scale-v2`: HUD scale

- On the page: Size of the in-game HUD, from 25% to 250%. Menus have separate sizes. Takes effect at the next start.
- A slider with 46 choices, in 5% steps. The stored value is the slider part, rather than the percentage shown on the page.
- A missing new key reads the older numeric `ui-scale` value with its original 50–150% interpretation, or Main's named 50–100% choices, preserving the player's HUD size. An explicit new key wins. The independent `menu-scale` key retains its original units and precedence.

### Existing-menu sizes

- `barter-menu-size-v1`: Buying and selling size.
- `repair-menu-size-v1`: Repair menu size.
- `workbench-menu-size-v1`: Workbench menu size, including campfires and reloading benches.
- Each slider has 21 choices from 50% to 150%, in 5% steps, saved independently. Missing values use 100%. These sizes are for the ordinary menus; their text, row layout, scrolling and hit geometry must remain aligned.
- [qualification] Settings/migration controls are authored but unexecuted. Actual menu rendering/drag integration is pending its owning consumer qualification; these settings alone do not establish working menu sizing.

