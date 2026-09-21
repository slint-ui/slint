<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->
<!-- cspell:ignore devicectl iphoneos xcresult xcresulttool -->

# iOS Native Overlays in Slint

This proof of concept keeps the text, caret, and selection highlight rendered by Slint.
A transparent UIKit `UITextInput` supplies the keyboard, selection handles, loupe, and edit menu.
It maps UIKit text positions to the Slint `TextInput` through Slint's text layout APIs.

The app includes a pure UIKit multiline editor beneath the Slint-backed editor.
Use both fields to compare selection, keyboard trackpad movement, and text input on the same device.

The UIKit inputs consume touches over the single-line and multiline fields.
This prevents Winit's host view from claiming first-responder status for the same touch.
The underlying Slint inputs are hidden from accessibility because the UIKit inputs represent them.

A transparent UIKit button presents a native menu beneath a Slint-rendered button.
The second field demonstrates wrapped multiline editing and responder transfer without dismissing
the keyboard.
It sits in a Slint `Flickable`: drag the exposed area below the field, or use the
scroll buttons, to move it while a native selection is active. The UIKit clip follows the Slint
viewport and passes empty-area touches through to Slint.
Both fields keep their text, carets, and selection highlights rendered by Slint.

See [Production Readiness](PRODUCTION.md) for the supported behavior, known gaps,
proposed architecture, and test plan.

## Build and Run

Generate the Xcode project:

```sh
cd tests/manual/ios-native-overlays
xcodegen generate
```

Select an attached iPhone and run the `SlintNativeOverlays` scheme in Release.
The UI tests verify native text delivery, model synchronization, responder transfer,
native text selection, keyboard trackpad endpoints, and the UIKit context menu.
The focused scroll test selects multiline text and drags the Slint `Flickable`.
It checks that the native editor moved while retaining keyboard focus.
It then verifies that typing replaces the selected word.
The keyboard trackpad tests compare final caret positions.
They don't measure the live movement curve or input latency.

## Run the UI tests from the command line

Connect and unlock the iPhone before starting a test run.
The phone must trust the Mac and have Developer Mode enabled.
Xcode waits with `Unlock iPhone to Continue` if the device locks before launch.

List the available devices and copy the iPhone identifier:

```sh
xcrun devicectl list devices
```

From the repository root, set the device identifier and Apple Developer Team ID:

```sh
export SLINT_DEVICE_ID="00000000-0000000000000000"
export SLINT_TEAM_ID="YOUR_TEAM_ID"
export SLINT_DERIVED_DATA="/private/tmp/slint-native-overlays-derived"
```

Generate the project and run the complete Release UI-test suite:

```sh
cd tests/manual/ios-native-overlays
xcodegen generate
xcodebuild test \
    -project SlintNativeOverlays.xcodeproj \
    -scheme SlintNativeOverlays \
    -configuration Release \
    -destination "id=$SLINT_DEVICE_ID" \
    -derivedDataPath "$SLINT_DERIVED_DATA" \
    DEVELOPMENT_TEAM="$SLINT_TEAM_ID" \
    CODE_SIGN_STYLE=Automatic
```

Use `-only-testing` to run a focused test:

```sh
xcodebuild test \
    -project SlintNativeOverlays.xcodeproj \
    -scheme SlintNativeOverlays \
    -configuration Release \
    -destination "id=$SLINT_DEVICE_ID" \
    -derivedDataPath "$SLINT_DERIVED_DATA" \
    -only-testing:SlintNativeOverlaysUITests/NativeOverlayTests/testMultilineSelectionFollowsSlintScroll \
    DEVELOPMENT_TEAM="$SLINT_TEAM_ID" \
    CODE_SIGN_STYLE=Automatic
```

After a successful build, use `test-without-building` for additional focused tests:

```sh
xcodebuild test-without-building \
    -project SlintNativeOverlays.xcodeproj \
    -scheme SlintNativeOverlays \
    -configuration Release \
    -destination "id=$SLINT_DEVICE_ID" \
    -derivedDataPath "$SLINT_DERIVED_DATA" \
    -only-testing:SlintNativeOverlaysUITests/NativeOverlayTests/testVerticalKeyboardTrackpadEndpointMatchesUIKit \
    DEVELOPMENT_TEAM="$SLINT_TEAM_ID" \
    CODE_SIGN_STYLE=Automatic
```

Xcode prints the `.xcresult` path at the end of each run.
Export retained screenshots and measurement attachments with:

```sh
xcrun xcresulttool export attachments \
    --path "/path/to/Test-SlintNativeOverlays.xcresult" \
    --output-path "/private/tmp/slint-native-overlay-results"
```

## Leave the app ready for use

XCTest can terminate the app or leave the test runner in front.
Install and launch the built Release app after the final test run:

```sh
export SLINT_APP_PATH="$SLINT_DERIVED_DATA/Build/Products/Release-iphoneos/SlintNativeOverlays.app"

xcrun devicectl device install app \
    --device "$SLINT_DEVICE_ID" \
    "$SLINT_APP_PATH"

xcrun devicectl device process launch \
    --device "$SLINT_DEVICE_ID" \
    --terminate-existing \
    dev.slint.native-overlays
```

Leave the phone unlocked, awake, and showing `Native Overlays`.
Dismiss system alerts and finish phone calls before handing the device to another person or an
automation run.
Leave the app on its initial screen for automation because each UI test establishes its own focus
and keyboard state.
For manual inspection, tap either multiline field and long-press the keyboard space bar to compare
trackpad behavior.

The command-line tools can install and launch the app but can't perform arbitrary touches.
Use the XCTest cases for repeatable interaction or operate the unlocked phone manually.
