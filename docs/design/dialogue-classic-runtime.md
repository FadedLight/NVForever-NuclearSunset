# Supplied Classic dialogue casing

Classic reads an optional locally supplied blank template at
`Textures/Interface/NVForever/Dialogue/classic.png` through `GameData`. The calibrated
layout is 1447×1087. The artwork remains private runtime data; it is not bundled in
this repository or copied into the player's data by this implementation.

The real supplied casing, lamps, speaker and controls remain opaque and retain their
original colours. A bounded connected-black mask clears the four actual rounded
apertures in memory: the left scene window, upper-right NPC text screen, lower-right
response screen and lower-left caps readout. The original PNG is never edited and no
transformed image is saved. This is a blank-template route, not a general screenshot
content remover; different dimensions or absent apertures are refused.

Eight source bands reflow the actual hardware for a landscape safe canvas. The caps
bezel is widened using the installed Font 5's complete signed-integer width at a
12-logical-unit minimum, with separate Review/grille and Barter pieces. No cash value
is shortened or silently replaced with zero. Unknown cash remains blank. A small
canvas or unsuitable font that cannot accommodate these regions is refused instead
of shrinking targets or silently making numbers unreadable.

The left aperture is the real world camera's viewport. The dialogue lease admits only
a live perspective player camera on the current primary window and only an unowned
full-window viewport, or the exact viewport that this lease previously wrote. Closing,
pausing, focus loss, disabled touch dialogue, camera replacement and unavailable data
retire the lease. Restoration changes only the viewport still owned by this lease;
a foreign viewport, explicit reset, transform or default-UI marker is preserved. A
foreign takeover is not immediately reacquired while the same Classic view stays open.
Field Notes retains its separate side-framing policy; the stored side setting does
not move Classic, Normal or the actual device presentations.

The upper-right screen consumes the authoritative last-spoken-line state, retaining
that actual text while responses are offered. Its independent scroll owner is not a
Skip action while choosing; during speech it uses the existing guarded Skip action.
The lower-right screen uses the existing offered choices, requirements, selection,
release guards, optional confirmation and scroll/reveal consumers. All interactive
rows and the real Barter hit target retain 48-logical-unit minima. Wrapped text can
make a row taller; the template does not promise that all choices fit without scrolling.

Barter is published only when the canonical model provides a current merchant/list
target. It uses the existing guarded release and action consumer, with no separate
trade interface. Otherwise the small red button is dimmed and has no action. Review
is decorative. The installed native Font 5 and existing dark-screen undercoat provide
live content; no default-font stand-in is reported as a ready Classic view.

The old custom backing, tint, accent and frame-opacity choices remain stored for the
configurable Field Notes treatment. They do not fade or recolour real hardware. Text
sizes and the saved confirmation mode still apply to Classic.

`Cache::prepare` retries while `GameData` is absent, then reads and decodes once. A
presentation/data reload can explicitly reset the caches. Ordinary frames do not read
the file again. `Surface` describes actual prepared art/font/geometry; it alone is not a rendered-frame
claim. Resolve keeps its camera proposal private. The ordered Admit phase validates
current ownership and writes the actual viewport before either UI painter selects a
view; only then does `Frame.classic` publish that applied identity. Before Scripts,
validation also withdraws last frame's identity if a foreign camera change made it
stale, so a terminal raw touch cannot use it. `ready_for` requires the current matching
committed lease. CameraUpdate remains in its ordinary PostUpdate schedule. A later
foreign takeover is respected and retired at the next admission, rather than revoking
the selected UI after it has already been painted.

The normal presenter can temporarily display genuine Normal dialogue if Classic is
unavailable, preserving the stored Classic request; a capture must refuse that fallback
as Classic success. All touch styles and recovery share one explicitly targeted
full-window UI camera, whose lifetime follows available dialogue/menu content. Its
existence does not assert side framing or a Classic world lease, and it requires
neither ScriptState nor a player camera merely to display ordinary dialogue. Foreign
default-UI markers remain untouched.

Source controls cover the decoder and runtime VFS, real font preparation, passive atlas
pieces, geometry, camera lease cleanup/foreign ownership and raw-contact retirement.
An ordered handoff control changes the actual Camera target, viewport or activity
between Resolve and Admit and checks the real Classic/Normal painter selection. The
CPU scene deliberately publishes authored `Camera.computed.target_info` for scheduling
and Taffy layout; it does not run or prove the actual CameraUpdate/window/GPU provider.
A second control changes the actual camera before Scripts and verifies that the old
contact cannot answer through last frame's publication. These CPU controls do
not establish real GPU rendering, original-game parity or physical handset acceptance.
