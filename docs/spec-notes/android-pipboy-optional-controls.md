# Optional Pip-Boy touch controls

[documented: engine/nvforever/src/touch/pipboy_drag.rs; engine/nvf-play/src/pipboy/gesture.rs] The existing bottom handle drives the actual first-person Pip-Boy clip through ArmGesture. Its release settles to an endpoint, cancellation returns to the original endpoint, and the existing menu consumer owns opening/closing. This path predates this extension.

[documented: engine/nvforever/src/touch/pipboy_model.rs; engine/nvforever/src/pipboy_arm.rs] The physical ScrollKnob has a target derived from the current CPU-posed model. Its vertical drag already uses 20 logical pixels per line and the shared late-checked ListScrollRequest. No guessed wheel bone, mesh or fixed screen rectangle is introduced. An unavailable/malformed/unprojected wheel remains unavailable.

[design] Independent saved switches enable left and right downward dismissal handles and an explicit close button. Their 48-logical-pixel regions sit inside all four published system insets. Existing painted model controls, menu canvas, scrollbar, tray and active gestures retain priority. A new contact must start in a configured unclaimed region. A downward drag reverses while held; releasing past halfway closes, cancellation returns open. The ordinary bottom handle remains separately controlled by its existing preference.

[design] Where the real worn first-person rig and clip are available, all dismissal handles use that same reversible arm animation. A screen-only or unavailable-rig menu has no invented arm: a deliberate downward swipe or close tap requests the ordinary guarded native close. The adapter clears on resize, inset changes, focus/suspend, controller or modal takeover and never synthesizes physical key/mouse state.

[design] Show Pip-Boy button graphics changes paint only. When off, overlay icon/background/label nodes are not drawn, while the exact same targets, bindings, ownership, clipping, cancellation and native action dispatch remain active. This is independent of disabling a feature. Native game-model parts are not hidden. Gameplay settings/help relocation is a separate change.

[design] The physical model wheel switch only gates its touch interaction; physical mouse/controller scrolling and generic on-screen scrollbars remain unchanged. Disabling it during a drag retires that contact and its queued lines; reenabling requires a fresh Started.

[hypothesis: qualification] New source fixtures must exercise actual projected wheel-to-list state, actual arm/menu consumer endpoints, visible/invisible equivalent hit dispatch, saved settings round trips and cancellation/owner coexistence. Compilation, runtime and phone acceptance remain pending until separately executed.

[design] Added dismissal contact IDs use a fixed 32-slot mask shared with the existing post-selector UI-focus publisher. Overflow consumes further raw edges until actual touch neutral; release/cancel edges remain masked. This does not suppress physical mouse input or fabricate release edges. A graphics toggle updates color alpha in place, without despawning its graphic nodes, changing visibility, or retiring the captured contact.

[design] Overlay roots explicitly target a validated full-primary-window camera, preserving any foreign default-UI owner. Logical window target positions/sizes and font/border sizes are divided by actual UiScale before layout, so their computed physical bounds agree with the raw window-logical targets at non-unit UI scales. The visual switch still only updates alpha on those same nodes. Dismissal's captured window stamp includes the actual Window Entity as well as logical/physical size and density; a same-size replacement cannot inherit a pending close from the old window.

[design] Model buttons and the physical wheel also retain a fixed 32-slot
window/contact-ID mask. Successful release, cancellation and wheel Off/On do not
turn an owned tail into a generic UI click. The existing post-selector publisher
uses only the current primary window's records; replacement windows cannot inherit
another window's same-numbered contact. Overflow refuses further model effects for
that window until actual touch neutral. It does not clear physical input or suppress
unrelated pointer IDs. Before InputSystems, neutral additionally requires the sole
reader's unread batch to be empty. The model request stamp includes actual window
entity, logical/physical size and density and is checked again at dispatch; queued
list-scroll application repeats the window check at its existing effect consumer.
Model target publication, live admission and both effect boundaries additionally
require the actual world camera destination to normalize to that primary window.
Primary and an explicit reference to the same window are equivalent, including a
change between those aliases during a contact. An offscreen image or another
window is not evidence of presentation in this window, even at the same dimensions.
This does not constrain internal render-resolution overrides, viewport or jitter.
Page commands keep the existing PipBoyInput route. Source controls for actual parent
input, Taffy/UI focus, late-window refusal and neutral recovery remain unexecuted
until separately qualified; they do not establish native XML or phone acceptance.
