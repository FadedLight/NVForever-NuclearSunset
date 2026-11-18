# Android launcher identity and diagnostics builds

Clean-room scope: this note describes NVForever's own Android port design, not the original game. Here `[hypothesis]` marks a port design proposal or pending device acceptance, and `[documented]` cites repository source, an owner request, a pinned dependency or public platform documentation; neither is a claim about Fallout: New Vegas (see the [classification](../design/android.md#clean-room-classification-of-the-ports-constants-and-behaviour)).

## Sources and intended behavior

- [documented] The project's own launcher artwork is
  `engine/nvforever/assets/icon.png`; `engine/nvforever/src/window_icon.rs`
  embeds it for desktop. Android adaptations must derive from that asset.
- [documented] Android adaptive icons use 108 dp foreground/background layers,
  with important artwork inside the central 66 dp safe zone. Launchers may apply
  different masks. Android 13 supports a monochrome layer for themed icons.
  [Android adaptive icons](https://developer.android.com/develop/ui/compose/system/icon_design_adaptive).
- [hypothesis] Center the complete canonical artwork within the 66 dp safe circle on
  an opaque near-black adaptive background. Supply legacy density variants and
  an alpha-derived monochrome layer; do not redraw the brand or use game art.
  `tools/android/LauncherIcons.java` is the reproducible source adaptation.
- [documented] Gradle can expose build-time settings through generated
  `BuildConfig` fields and Android resources; AGP requires explicit BuildConfig
  generation when enabled. [Gradle build tips](https://developer.android.com/build/gradle-tips),
  [BuildFeatures](https://developer.android.com/reference/tools/gradle-api/8.4/com/android/build/api/dsl/BuildFeatures).
- [hypothesis] `-PnvfDiagnostics=true` explicitly opts a development APK into
  diagnostics UI. The default is false. Values other than `true`/`false` are
  errors. This option is independent of the optimized Rust `android-dev` profile.
  The generated resource also records the request for inspection in the APK.
- [documented] `LanDiagnostics.available` currently requires Android's
  `ApplicationInfo.FLAG_DEBUGGABLE`, and the launcher uses that method to admit
  its controls. Retain that safeguard in addition to the explicit build option.
  `GameActivity.renderDiagnosticsBuildType` reports the actual Android debug
  flag, independently of `renderDiagnosticsUi`'s explicit capability request.
  The native render-admission observer requires both; neither describes Rust
  optimization.
- [hypothesis] Disabling diagnostics must hide all diagnostics entry points and
  their input regions, without erasing pairing, pending reports, saves or local
  crash logs. Enabling the option never grants upload consent: existing private
  LAN validation and explicit consent still apply. Android release builds remain
  ineligible for LAN support even if the option is requested.
- [documented] Local native crash/event logs are independent of the Java LAN
  worker (`engine/nvforever/src/crash.rs`, `events_log.rs`). Their collection must
  remain available when diagnostics UI is disabled.
- [documented] The current Android diagnostics entry point is the launcher's
  LAN test-logs control (`LauncherActivity`). The touch overlay in `touch.rs`
  and `touch/` has no diagnostics button or reserved diagnostics hit rectangle.
  This change adds no in-game diagnostics control.

## Validation boundary

- [hypothesis] Inspect packaged icon references, each density, adaptive masks
  and monochrome alpha against the canonical source. Compile all production
  Java and exercise both diagnostic flag values through the real Android
  packaging path. Verify invalid option rejection, debug/release safeguard,
  final merged manifests, and unchanged application/update/save identity.
- [hypothesis] Keep implementation checks separate from device acceptance.
  Launcher appearance and touch ownership on a physical device require a device
  run; source or package inspection alone does not establish those outcomes.
