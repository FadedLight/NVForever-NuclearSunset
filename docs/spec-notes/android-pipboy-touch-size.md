# Android Pip-Boy presentation size

[hypothesis: NVForever touch presentation policy] A separate saved size controls the
Pip-Boy device and its flat-screen fallback. Android starts at 110%; the bounded
range is 85–135%. Desktop starts at the existing 100%. This preference does not
change general HUD/menu scale, world field of view, inventory identity or actions.

[documented: player_body.rs::place_lens, pipboy_arm.rs::Posed and
pipboy.rs::crop_point] Device input is projected through the actually posed mesh;
flat input uses the actual laid-out image bounds. Apply size to the arm's existing
Pip-Boy lens and to the fallback's visible bounds, rather than multiplying input
coordinates independently. Use the same lens during covered warm poses. The
device's texture allocation remains unchanged.

[hypothesis: input ownership policy] A live preference change cancels pending
virtual item/drag intent before projected picking. A held contact must not become
a fresh click in moved content. Physical mouse/controller state is not cleared.

[documented: pipboy/touch_presentation.rs::cancel_changed] Before the pointer
driver, a presentation stamp compares logical/physical window size, density,
common safe insets, Pip-Boy ratio, menu units and UI scale. Changed or invalid
metadata cancels queued projections and virtual intent before they can release
on old bounds. A first valid stamp preserves fresh input. The actual laid-out
node's geometry check remains an additional guard, rather than the only signal.
Platform inset publication precedes this invalidation. Authored regression
fixtures exercise the actual flat image/layout/projector and pointer driver;
they have not yet been compiled or executed in this candidate.

[hypothesis: qualification] Verify independent INI persistence, original 100%
projection, device and flat inverse mappings at multiple sizes/densities, warm
pose agreement, and cancellation without item use. Compilation, source tests and
physical readability are separate; this note does not claim phone acceptance.
