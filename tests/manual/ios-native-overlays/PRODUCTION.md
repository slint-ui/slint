<!-- Copyright © SixtyFPS GmbH <info@slint.dev> -->
<!-- SPDX-License-Identifier: MIT -->

# Production Readiness

## Purpose

This prototype explores native iOS text interaction over a Slint-rendered text field.
Slint remains responsible for the text, caret, and selection highlight.
UIKit provides the keyboard, edit menu, selection handles, loupe, and keyboard trackpad.

The approach is viable, but the current implementation isn't a reusable Slint feature.
It proves the platform integration and identifies the work needed for production.

## What The Prototype Proves

The prototype demonstrates these behaviors on a physical iPhone:

- A transparent `UIView` can implement `UITextInput` over a Slint `TextInput`.
- Slint and UIKit content can occupy the same window and overlap.
- UIKit can provide native keyboard input without drawing replacement text.
- UIKit can provide the edit menu, selection handles, and loupe.
- Slint can continue drawing the caret and selection highlight.
- Single-line and multiline inputs can transfer first-responder status.
- Selection handle movement can update Slint while the handle is still moving.
- A Slint-rendered button can use a transparent native button to present a `UIMenu`.
- A pure `UITextView` can run beside the bridge for device comparisons.

Focused tests on a simulator and physical device demonstrate a selected field following a Slint
`Flickable`.
Its UIKit handles remain aligned and clipped to the Slint viewport.

The automated tests also compare the final caret displacement after horizontal and vertical
keyboard trackpad gestures.
Those tests don't measure the live cursor trajectory.

## Current Architecture

`SlintTextInputView` implements `UITextInput` and stores a UIKit mirror of the text and selection.
The native view forwards changes through C functions into the Slint component.

The Rust bridge performs four tasks:

1. Convert offsets between UIKit UTF-16 units and Rust UTF-8 byte offsets.
2. Read caret rectangles from Slint's text layout.
3. Hit-test a UIKit point against Slint's text layout.
4. Update Slint's text, selection, focus state, and selection color.

Text updates and selection updates use separate calls.
Caret movement therefore doesn't reassign or reshape the complete text.

The native context-menu button is independent from the text-input bridge.
It proves that a native control can present UI from the position of a Slint-rendered control.

## Readiness Summary

| Area | Status | Evidence Or Gap |
| --- | --- | --- |
| Native keyboard input | Demonstrated | Device UI tests insert and delete text. |
| Copy, cut, paste, and select all | Demonstrated | UIKit actions operate on the mirrored storage. |
| Selection handles and loupe | Demonstrated | Device tests move leading and trailing handles. |
| Live multiline selection | Demonstrated | Slint updates while a handle remains pressed. |
| Keyboard trackpad endpoint | Demonstrated | Slint and UIKit finish within two UTF-16 units. |
| Keyboard trackpad trajectory | Unverified | Tests don't record position or latency over time. |
| Arbitrary fonts | Partially demonstrated | Slint keeps its font, but UIKit geometry depends on Slint's layout APIs. |
| Programmatic text changes | Missing | Slint changes don't update the native storage. |
| Programmatic selection changes | Missing | Slint changes don't update UIKit's selection. |
| Marked text and IME composition | Partial | Text is mirrored, but Slint doesn't expose marked-text styling. |
| Bidirectional and vertical text | Missing | Selection rectangles assume left-to-right horizontal text. |
| Scrolling field geometry | Partially demonstrated | Focused simulator and physical-device tests move one field through a Slint `Flickable`; arbitrary transforms and nested clips remain untested. |
| Other dynamic layout | Missing | Rotation, resize, safe-area, scale, and animation changes aren't covered. |
| Accessibility | Unverified | VoiceOver and other assistive technologies need a dedicated pass. |
| Multiple windows and teardown | Missing | A process-global manager owns one window's overlays. |
| iOS 16 selection appearance | Missing | Native highlight suppression uses an iOS 17 protocol. |
| Performance limits | Unverified | No callback latency or dropped-frame thresholds exist. |

## Production Gaps

### Stable Slint API

The Rust bridge uses `i-slint-core` item-tree and text-layout APIs.
It also finds editors by their order in the item tree.
These APIs and identifiers aren't stable application contracts.

A production implementation needs an internal Slint text-input adapter with explicit handles.
The adapter should expose text, selection, caret geometry, hit testing, focus, and colors.
The iOS backend should use that adapter instead of traversing the item tree.

### Bidirectional State

The current bridge treats UIKit as the source of editing changes after installation.
If application code changes the Slint text or selection, UIKit keeps stale state.

Use a versioned state object with these fields:

- text
- selection anchor and focus
- marked-text range
- editing revision
- active editor identity

Changes from either side should update the other side without producing a feedback loop.
The active native view should refresh before UIKit requests geometry or text.

### Native View Lifetime

The prototype creates one native view per example field and keeps a process-global manager.
It doesn't remove overlays when the Slint window closes or recreates its native window.

Use one bridge object per Slint window.
Create one reusable `UITextInput` view for the active Slint field.
Move and reconfigure that view when focus changes.
Destroy it with the window adapter.

This model removes per-field native views and most focus-switching branches.

### Geometry And Scrolling

The scrollable example updates its native editor frame whenever the Slint `Flickable` changes its
content offset.
A transparent clipping view uses the Slint viewport rectangle.
It passes touches outside the editor through to Slint.
The focused UI test selects a word and performs a real drag in the exposed Slint content.
It verifies that the editor moves while the keyboard stays active.
It captures the aligned selection handles and highlight.
Typing after the scroll still replaces the selection.

This is one axis-aligned example. Rotation, resize, safe-area changes, nested scrolling, arbitrary
transforms, animations, and scale changes can still move a Slint field without a proven matching
UIKit update.

Update native geometry after Slint layout and before UIKit interaction begins.
Clip the overlay to the same viewport as the Slint field.
Convert points through the window transform instead of assuming a fixed logical coordinate system.

The bridge must also follow a field inside a scrolling or transformed container.

### Touch Arbitration

The transparent input must remain the hit-test target while `UITextInteraction` owns the text
gestures.
Its touch callbacks intentionally consume events without calling the superclass implementation.
Removing them lets the Winit host interfere with first-responder state and native selection.

A production bridge should express this behavior in one platform-owned view.
Device tests must cover typing, double-tap selection, handle movement, and trackpad movement after
any hit-testing change.

### Text Layout Contract

UIKit requests caret rectangles, selection rectangles, hit-test positions, writing direction,
and text ranges.
The prototype returns enough geometry for the tested left-to-right examples.

A production adapter needs:

- one selection rectangle for every covered visual line
- leading and trailing writing-direction information
- affinity at wrapped-line boundaries
- grapheme-safe offset conversion
- bidirectional text support
- vertical text behavior, or an explicit unsupported result
- geometry for empty lines, trailing newlines, and scrolled content

The adapter should avoid exposing renderer-specific types to UIKit code.

### Marked Text And System Features

Input methods use marked text while composing characters.
The prototype stores marked ranges but Slint doesn't draw a marked-text treatment.

Production support must cover:

- Chinese, Japanese, and Korean composition
- combining characters and emoji sequences
- automatic correction and replacement ranges
- dictation
- hardware keyboards
- undo and redo
- writing tools
- secure text entry
- password-manager and one-time-code behavior

The Slint field must display composition without changing its font or text rendering.

### Selection Appearance

Slint owns the selection fill.
UIKit owns the handles and loupe.
The bridge tints the native controls from the Slint selection color and hides UIKit's selection
fill on iOS 17 and later.

The application still targets iOS 16.6.
The public highlight-view protocol used by the prototype isn't available there.
Choose one of these policies before production:

- raise the minimum version to iOS 17
- accept UIKit's selection fill on iOS 16
- add a supported iOS 16 implementation after device validation

Don't depend on UIKit private view-class names.

### Accessibility

The native overlay currently represents the Slint field in the UIKit accessibility tree.
That avoids exposing two editable elements, but the full behavior hasn't been tested.

Validate:

- VoiceOver reading and editing
- rotor navigation
- Switch Control
- Full Keyboard Access
- Voice Control
- larger accessibility text sizes
- reduce-motion and contrast settings
- focus transfer between Slint and UIKit controls

The production bridge needs one authoritative accessibility element per field.

## Proposed Production Shape

Add a platform text-input session owned by the window adapter.
The session should connect one active Slint text editor to one native input view.

The Slint side should provide an adapter similar to this conceptual interface:

```text
snapshot() -> text, selection, marked range, traits, revision
replace(range, text, selection)
set_selection(anchor, focus)
caret_rect(offset, affinity)
selection_rects(range)
position_at(point) -> offset, affinity
geometry() -> window rect, clip rect, transform
```

The iOS side should:

1. Activate a session when a Slint text field receives editing focus.
2. Mirror the latest snapshot into the native view.
3. Forward UIKit edits as atomic text and selection changes.
4. Forward caret-only movement as selection changes.
5. Query Slint for all geometry.
6. End the session when focus or the window disappears.

Keep native context menus as a separate overlay service.
Menus apply to controls beyond text input and shouldn't depend on the text session.

## Test Plan

### Offset And State Unit Tests

Test UTF-8 and UTF-16 conversion without a device.
Include ASCII, accented text, combining marks, emoji, skin-tone modifiers, flags,
zero-width joiners, and malformed boundary requests.

Test state synchronization with a fake Slint adapter.
Cover native edits, programmatic Slint edits, selection-only changes, marked text,
stale revisions, and focus transfer.

### Geometry Tests

Use deterministic text layouts to test:

- every caret offset
- wrapped-line boundaries
- empty lines and trailing newlines
- multiline selection rectangles
- right-to-left and mixed-direction lines
- alignment and padding
- scrolling and clipping
- transforms and display scale

### Device Interaction Tests

Keep the current physical-device tests for keyboard input, menus, focus, handles, multiline
selection, and the scrolling field.
Add tests for programmatic updates, rotation, background and foreground transitions, nested and
animated scrolling, hardware keyboards, and window recreation.

Run input-method tests with representative Chinese, Japanese, Korean, Arabic, Hebrew,
and Indic keyboards.
Include dictation and emoji input as manual device cases where automation isn't reliable.

### Keyboard Trackpad Tests

Endpoint displacement is useful but insufficient.
Instrument both the Slint bridge and a pure `UITextView` to record every selection change with a
monotonic timestamp.

Replay the same keyboard gesture and compare:

- caret position over time
- distance per input sample
- time to first movement
- maximum callback gap
- overshoot and reversal behavior
- final position
- dropped or coalesced updates

Store the samples as an XCTest attachment.
Plot both trajectories on the same axes.
Fail on defined thresholds instead of visual judgment.

Record a 120 Hz device video for manual confirmation.
The plist keeps the application eligible for the device's higher refresh rate.

### Visual Tests

Capture idle, focused, selected, composing, and handle-drag states.
Compare the Slint-rendered text before and during native interaction.

Cover multiple fonts, weights, sizes, letter spacing, line heights, colors,
and selection colors.
The acceptance criterion is no text movement when native editing begins or ends.

### Performance Tests

Measure on supported physical devices.
Define thresholds for:

- selection callback latency
- caret redraw latency
- dropped frames during trackpad movement
- text-layout queries per input sample
- allocations during steady caret movement
- activation and focus-transfer time

Assert that selection-only movement never reassigns the complete text.

### OS And Device Matrix

Test the minimum supported iOS version, the current release, and the next release beta.
Include a 60 Hz phone, a ProMotion phone, an iPad, and at least one older supported device.

## Delivery Stages

### Stage 1: Harden The Prototype

- Add bidirectional text and selection synchronization.
- Reuse one native text-input view.
- Update geometry after layout changes.
- Add lifecycle cleanup.
- Decide the iOS 16 selection policy.
- Add trajectory instrumentation.

### Stage 2: Add A Slint Adapter

- Replace item-tree traversal with an explicit internal adapter.
- Define offset, affinity, marked-text, and geometry contracts.
- Add renderer-independent unit tests.

### Stage 3: Complete Platform Behavior

- Add IME, bidirectional text, accessibility, scrolling, and lifecycle coverage.
- Validate system writing features and hardware keyboards.
- Set performance thresholds from physical-device measurements.

### Stage 4: Integrate And Stabilize

- Put the bridge behind an experimental feature or platform option.
- Run the device matrix in scheduled CI or a device lab.
- Document supported iOS versions and known limitations.
- Promote the adapter only after its state and geometry contracts stabilize.

## Suggested Pull Request Scope

Keep an initial review focused on the manual prototype and its tests.
Don't present the current `i-slint-core` calls as a stable API.

A follow-up implementation pull request should introduce the internal adapter,
one reusable native session, lifecycle management, and bidirectional synchronization.
IME and accessibility work can follow after the adapter contract is reviewable.

The current prototype is strong enough to justify that work.
It isn't ready to ship as the iOS text-input implementation without the gaps above.
