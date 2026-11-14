# Android native-window loading cover

[observed] The R6 phone report is a visible main menu with neither touch nor virtual
buttons responding. Automated native input tests passed, but did not exercise the
Android window that supplies the loading-cover acknowledgement.

[documented] Android's NativeActivity takes its window surface and input queue.
ViewRootImpl skips Java drawing for an application-owned surface. A child added
with addContentView therefore cannot provide the hardware frame-commit callback
used by the cover protocol. See the Android 15 implementations of
[NativeActivity](https://raw.githubusercontent.com/aosp-mirror/platform_frameworks_base/android15-release/core/java/android/app/NativeActivity.java)
and [ViewRootImpl](https://android.googlesource.com/platform/frameworks/base/+/refs/heads/android15-release/core/java/android/view/ViewRootImpl.java),
and the hardware-only [frame-commit contract](https://developer.android.com/reference/android/view/ViewTreeObserver#registerFrameCommitCallback(java.lang.Runnable)).

[observed] The R6 source requires both that cover commit and a matching native
content-submission witness before revealing content. An absent cover commit keeps
LoadingState blocking and the shared PlatformInputGate closed, including native
menu pointers, menu action buttons and wrapper controls. This source defect
matches the reported failure; physical confirmation of its repair remains required.

The recovery uses a separate ordinary application dialog window for the cover.
Configure hardware acceleration before its content view. Keep it nonfocusable and
nontouchable so the native activity retains focus and its original surface/input
queue. Remove dim-behind. The platform gate continues quarantining game input;
the cover does not manufacture input events or consume native queue ownership.
Window lifecycle and generation checks reject callbacks from dismissed covers.
Real cover frame commitment and the native request/epoch/surface/content witness
remain required; no timer, posted callback or unconditional gate bypass stands in
for either. Back/refusal recovery remains in a separate interactive dialog.

[documented] Dialog creates an independent PhoneWindow and attaches its decor
through WindowManager. Relevant contracts are
[Dialog](https://raw.githubusercontent.com/aosp-mirror/platform_frameworks_base/android15-release/core/java/android/app/Dialog.java)
and [WindowManager flags](https://developer.android.com/reference/android/view/WindowManager.LayoutParams).

The labeled Java recovery APK retains the exact qualified R6 native library pair.
Its SOURCE identifies the Java/package revision; NATIVE-SOURCE identifies the
unchanged native CI revision. Local window-policy tests and SDK compilation do
not substitute for a fresh phone main-menu, interruption and loading test.
