# Mobile Pip-Boy model controls

[design] The three visible bottom controls get projected touch rectangles, at
least 48 logical pixels, centered on their actual posed meshes. The overlay labels
identify their sections and show a stronger border/fill while pressed. Overlapping
touch rectangles choose the closest center. A drag, model/geometry change, focus
loss, overlay or physical-input takeover cancels the contact.

[documented: pipboy_arm.rs::follow_pointer, touch/pipboy_model.rs] Projection uses
the same CPU-posed shapes and player camera as the device screen pointer. No
independent fixed-screen coordinates, global camera or mouse-button emulation
are introduced. Release queues one page request, consumed once after fresh
platform, modal, physical-owner and geometry checks before the normal Pip-Boy
input handler. Tray controls and existing item gestures take priority.

[documented: touch/pipboy_model.rs; pipboy/scrollbar.rs] The composed adapter uses
the same projection for the physical scroll knob. Its owned contact accumulates
vertical movement in 20 logical-pixel steps, queues bounded list-scroll requests,
and rechecks the contact's admission before dispatch. Rotating the knob does not
invalidate its own contact. Hidden, malformed or stale posed parts cannot publish
new input geometry. The screen footprint takes priority over enlarged buttons.

[hypothesis: qualification] Authored, unrun posed-camera App fixtures exercise button
projection at three densities, pressed nodes, section changes, drag cancellation,
late capture and stale geometry without replay. These tests do not substitute
for full wrapper arbitration or physical phone acceptance. Additional composed
fixtures cover knob-to-list routing, hidden/inactive parts, malformed skin data,
late ownership changes and actual parent closing-handle arbitration. Their source
presence does not establish a successful compiler or test run.
