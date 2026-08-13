// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

@TestOn('vm')
library;

import 'package:slint/slint.dart';
import 'package:test/test.dart';

const _app = '''
export component App inherits Window {
    width: 64px;
    height: 32px;
    background: root.tint;
    in-out property <color> tint: #ff0000;
    in-out property <int> clicks: 0;
    in-out property <string> typed: "";
    init => { keys.focus(); }

    keys := FocusScope {
        width: 100%;
        height: 100%;
        key-pressed(event) => {
            root.typed += event.text;
            accept
        }
        TouchArea {
            width: 100%;
            height: 100%;
            clicked => { root.clicks += 1; }
        }
    }
}
''';

void main() {
  // The software renderer is installed once for the isolate, so the whole
  // surface lifecycle is one test rather than a group with a fresh setUp.
  test('renders frames and routes input', () {
    final surface = SlintSurface();
    // Asking twice hands back the same surface rather than failing.
    expect(identical(SlintSurface(), surface), isTrue);

    surface.resize(64, 32);
    expect(surface.width, 64);
    expect(surface.height, 32);

    final app = loadSource(_app)..show();
    addTearDown(app.dispose);

    final frame = surface.render();
    expect(frame, isNotNull, reason: 'the first frame must be drawn');
    expect(frame!.length, 64 * 32 * 4);
    // The window is opaque red: RGBA, premultiplied.
    expect(frame.sublist(0, 4), [255, 0, 0, 255]);

    // Nothing changed, so the next frame is skipped.
    expect(surface.render(), isNull);

    surface
      ..dispatchPointer(PointerEventKind.pressed, x: 10, y: 10)
      ..dispatchPointer(PointerEventKind.released, x: 10, y: 10);
    expect(app['clicks'], 1);

    surface.dispatchFocus(focused: true);
    surface
      ..dispatchKey(KeyEventKind.pressed, 'h')
      ..dispatchKey(KeyEventKind.released, 'h')
      ..dispatchKey(KeyEventKind.pressed, 'i')
      ..dispatchKey(KeyEventKind.released, 'i');
    expect(app['typed'], 'hi');

    // A change that affects what is drawn produces a frame again.
    app['tint'] = '#0000ffff';
    final repainted = surface.render();
    expect(repainted, isNotNull);
    expect(repainted!.sublist(0, 4), [0, 0, 255, 255]);

    surface.dispose();
    expect(surface.render(), isNull, reason: 'a disposed surface draws nothing');
  });

  test('SlintKey.function covers F1 to F24', () {
    expect(SlintKey.function(1), '\u{f704}');
    expect(SlintKey.function(24), '\u{f71b}');
    expect(() => SlintKey.function(0), throwsA(isA<ArgumentError>()));
    expect(() => SlintKey.function(25), throwsA(isA<ArgumentError>()));
  });
}
