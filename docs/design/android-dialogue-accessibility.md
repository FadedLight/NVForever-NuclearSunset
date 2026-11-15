# Touch dialogue accessibility

[design: owner request, Android follow-up to #2436] Dialogue needs wide wrapped
choices, larger touch targets, independent choice/subtitle sizes, scrolling, and a
choice between direct taps and selecting before confirmation. This increment owns
the conversation view, not game dialogue rules or global HUD scaling.

[documented: `engine/nvf-play/src/dialogue.rs`, `dialogue_menu.rs`,
`tiles/dialog_menu.rs`] The runtime supplies ordered eligible choices. Every view
request carries a `ListId`; the conversation input rejects a request from another
conversation, response or choice list. Keyboard/controller confirmation and game
commands already pass through that input. The new view keeps that route and the
same indices, challenge information and said-before state.

[design] A persisted **Touch dialogue** setting enables a separate full-width Bevy
view (on by default for Android, off elsewhere). The ordinary hand-built and tile
views give way while this setting is on. Turning it off restores the existing view.
Choice and conversation subtitle text scales are separate bounded settings, 75–200%.
This view uses Bevy's UI text rather than the tile view's game-font glyph images;
the offered strings and their status still come from the same conversation model.
The view uses Bevy text with word-or-character wrapping, rows at least 56 logical UI pixels high, and a clipped
vertical viewport. The speaker and the offered text come from the existing model.
Scrolling does not select or answer a choice. Keyboard/controller selection brings
the selected row into view.

[design] **Select then confirm** is the touch default: a released tap selects a row,
then a separate confirmation target answers it. Confirmation starts unarmed and
requires that the selected row still matches the released selection. **Direct tap** answers on release.
Real mouse clicks and keyboard/controller choices keep their existing commands.
The conversation's speaking line can be skipped by a fresh released tap, not by
the finger that entered the conversation or ended the previous response.

[design] One fresh contact owns a gesture. A drag past eight logical window pixels,
a second contact, cancellation, focus loss, suspension, a resize, a changed list or
settings change cancels its activation. A release must land on the same visible
target and the same list. Input is ignored under pause, another game menu, character
creation or controller ownership. Touch controls underneath this view are hidden
and do not synthesize menu keys; raw touch remains available to the dialogue view.
The first touch restoring a controller-selected device cannot answer dialogue.

[documented: local Bevy 0.20.1 `ui_pointer.rs` and `ComputedNode`]
Touch positions are logical window pixels; UI hit boxes are physical pixels. Hit
testing uses the window scale and actual computed node transform/clipping. Scroll
offsets use the node's inverse scale factor, rather than guessing device density.

[design] The settings use the existing Settings/INI flow. No saves, quest state,
game data or control bindings are rewritten. Existing list stamps and runtime
eligibility remain authoritative, including while a new list appears during input.

[design] Qualification must cover actual TouchInput start/move/end/cancel routing,
fast taps, entry and list replacement, drag-away-and-back, scrolling, multi-touch,
focus/suspend/resize cancellation, both tap modes, selection/confirmation indices,
and settings persistence. Layout checks must cover wrapped long choices and
independent text sizes.

[qualification] The [R9 hosted CPU run](https://github.com/LootJunkieAzza/NVForever/actions/runs/43669164991/job/132675180379)
passed all thirty `dialogue_touch` cases: raw touch/release and scroll guards,
entry/list replacement, multi-touch and lifecycle cancellation, controller return,
both tap modes, persistence, wrapping, independent sizes and split-camera/layout
controls. This result applies to R9 source `28d5469`; the later shared touch-owner
filter is a separate integration change. Native/device rendering remains unqualified.

[design, Android compact choices] New Android settings default choice text to 85% (17 logical pixels); explicit saved choice and subtitle scales remain authoritative. Subtitles keep the 100% default. Desktop defaults and the desktop touch view retain their previous layout. No setting key changes or new gameplay state are introduced.

[design] Android's touch conversation panel uses 75% of the window height, 8-pixel outer padding, 4-pixel gaps and 48-pixel minimum choice/confirm targets. Choice padding is 6 pixels; wrapped text can grow the row and the existing viewport scrolls it. The previous touch layout remains the desktop profile. This density change is intended to expose more choices on short landscape screens while keeping full labels and the same offered choice order and guarded release actions. Existing text-size sliders remain 75–200%.

[qualification] All four compact-choice cases passed in that R9 run using production draw, Bevy text measurement and real UI layout. They check more whole visible choices, 48-pixel targets, wrapped text, raw release/scroll guards and explicit saved INI sizes. These are headless CPU/layout results, not native rendering or device usability acceptance.


## Dialogue appearance [design]

Enhanced dialogue keeps its own on/off switch, text scales, confirmation and
speaker-side framing. Appearance adds independently saved Minimal, Classic Glass,
Split Panel, Retro Console and Wrist Terminal choices, background/frame opacity,
and Show dialogue hints. These are original thin node/vector trims inspired by
interface styles; no reference pixels, game artwork or fonts are imported.

The HUD/interface palette is authoritative at full text brightness. Surfaces use
a dark tint and restrained alpha (default18% background/60% frame), with backing
capped at80% and thin frame opacity independent. A functional selection mark
keeps full HUD contrast even when both cosmetic opacities are zero. Text, required
feedback, NPC speech, choices and selected-action captions never inherit those
opacity sliders. The existing selected unmet-check red is semantic feedback, not
a theme override. Cosmetic borders follow frame opacity, while the existing
functional selection mark stays opaque red for a selected unmet check, even at
zero frame/backing opacity. Other selected marks use full HUD contrast. The
original requirement text remains at every opacity. Theme and opacity changes repaint existing entities without
changing targets, selection, scrolling, ownership or camera framing. Decorative
nodes ignore picking. Show dialogue hints affects only the explanatory gesture
caption; it cannot hide speech/choices/requirements/Confirm selection. Its layout
change invalidates a pending touch release through the existing guard. Fresh
neutral contacts remain usable with the independently enabled view.

A noninteractive live sample sits beside the selected appearance row's existing
settings note, only when the actual note region has at least78interface units
and every uncut measured note line fits beside it. Overlong words/reasons retain
the full-width note instead of being truncated to admit a sample. It shares the real
paint path, is absent from gameplay, and cannot cover rows, scrollbars, the rail
or Back. Narrow notes retain their full text and omit the sample. Generic palette
recolour runs before the appearance owner's paint, preventing a live palette
change from applying twice. New options use the existing INI settings consumer.

Authored controls exercise actual nodes/alpha, Taffy hit bounds, live settings,
scroll/selection preservation, hints off with real raw choices, both framing
sides, long/disabled choices, saved restoration and actual note-region sample.
All new controls remain uncompiled/unrun until an owner-admitted check. They do
not qualify phone pixels or installed mod/version/profile behavior.
