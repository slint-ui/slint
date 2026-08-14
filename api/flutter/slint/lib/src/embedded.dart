// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/// Embedded mode: Slint renders into a buffer you own.
///
/// See `api/flutter/rust/embedded.rs` for why this exists — in short, the Dart
/// VM does not run `main()` on the process main thread, which a native event
/// loop requires on macOS, and inside Flutter a second native window would not
/// compose with the widget tree anyway.
library;

import 'dart:typed_data';

import 'backend.dart';

/// What happened to the pointer. The numbers are the ABI, so keep them in step
/// with `slint_dart_embedded_pointer_event`.
enum PointerEventKind {
  pressed(0),
  released(1),
  moved(2),
  scrolled(3),
  exited(4);

  const PointerEventKind(this.code);
  final int code;
}

enum PointerButton {
  left(0),
  right(1),
  middle(2);

  const PointerButton(this.code);
  final int code;
}

enum KeyEventKind {
  pressed(0),
  repeated(1),
  released(2);

  const KeyEventKind(this.code);
  final int code;
}

/// The characters Slint uses for keys that carry no text of their own. They
/// come from the table in `internal/common/key_codes.rs`; pass them to
/// [SlintSurface.dispatchKey] as the key's text.
abstract final class SlintKey {
  static const backspace = '\u{0008}';
  static const tab = '\u{0009}';
  static const enter = '\u{000a}';
  static const escape = '\u{001b}';
  static const backtab = '\u{0019}';
  static const delete = '\u{007f}';
  static const shift = '\u{0010}';
  static const control = '\u{0011}';
  static const alt = '\u{0012}';
  static const capsLock = '\u{0014}';
  static const meta = '\u{0017}';
  static const space = '\u{0020}';
  static const upArrow = '\u{f700}';
  static const downArrow = '\u{f701}';
  static const leftArrow = '\u{f702}';
  static const rightArrow = '\u{f703}';
  static const insert = '\u{f727}';
  static const home = '\u{f729}';
  static const end = '\u{f72b}';
  static const pageUp = '\u{f72c}';
  static const pageDown = '\u{f72d}';

  /// `F1` through `F24`.
  static String function(int n) {
    if (n < 1 || n > 24) {
      throw ArgumentError.value(n, 'n', 'Slint has F1 through F24');
    }
    return String.fromCharCode(0xf704 + n - 1);
  }
}

/// A Slint surface that draws into a buffer instead of onto its own window.
///
/// The owner drives it: [resize] when the surface changes size, [render] once
/// per frame, [tick] to advance timers and animations, and the `dispatch*`
/// methods to deliver input.
///
/// Create this **before** loading any `.slint` file — the renderer has to be
/// the active platform before the first component exists — and call
/// `show()` on the instance you load so it attaches to the surface.
class SlintSurface {
  /// Install the software renderer. Doing this twice returns the same surface.
  factory SlintSurface() {
    backend.embeddedInit();
    return _instance ??= SlintSurface._();
  }

  SlintSurface._();

  static SlintSurface? _instance;

  int _width = 0;
  int _height = 0;

  int get width => _width;
  int get height => _height;

  /// Resize to [width] x [height] physical pixels. [scaleFactor] is the
  /// device pixel ratio, mapping those to the logical pixels `.slint` code
  /// sizes itself in.
  void resize(int width, int height, {double scaleFactor = 1.0}) {
    backend.embeddedResize(width, height, scaleFactor);
    _width = width;
    _height = height;
  }

  /// Draw the next frame, and return the pixels when anything changed.
  ///
  /// The result is only valid until the next [render] or [resize] — copy it
  /// if you need to keep it. Returns null when nothing needed repainting.
  Uint8List? render() => backend.embeddedRender(_width, _height);

  /// Advance timers and animations. Returns how long the caller may idle
  /// before the next update is due, or null when nothing is pending.
  Duration? tick() {
    final ms = backend.embeddedTick();
    return ms < 0 ? null : Duration(milliseconds: ms);
  }

  /// True while an animation is running, so keep asking for frames.
  bool get hasActiveAnimations => backend.embeddedHasActiveAnimations();

  void dispatchPointer(
    PointerEventKind kind, {
    double x = 0,
    double y = 0,
    PointerButton button = PointerButton.left,
    double deltaX = 0,
    double deltaY = 0,
  }) {
    backend.embeddedPointerEvent(kind.code, x, y, button.code, deltaX, deltaY);
  }

  void dispatchKey(KeyEventKind kind, String text) =>
      backend.embeddedKeyEvent(kind.code, text);

  /// Tell Slint whether the surface has keyboard focus, so text cursors blink
  /// and selections render the way the platform expects.
  void dispatchFocus({required bool focused}) =>
      backend.embeddedFocusEvent(focused);

  /// Release the pixel buffer. The Slint platform itself stays installed for
  /// the life of the process.
  void dispose() {
    backend.embeddedDispose();
    _width = 0;
    _height = 0;
  }
}
