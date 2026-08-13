// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slint_flutter/slint_flutter.dart';

const _app = '''
export component App inherits Window {
    in-out property <int> clicks: 0;
    in-out property <length> known-width: root.width;
    TouchArea {
        width: 100%;
        height: 100%;
        clicked => { root.clicks += 1; }
    }
}
''';

void main() {
  testWidgets('sizes the surface and forwards taps to Slint', (tester) async {
    late ComponentInstance app;

    await tester.pumpWidget(MaterialApp(
      home: Center(
        child: SizedBox(
          width: 300,
          height: 200,
          child: SlintView(load: () => app = loadSource(_app)),
        ),
      ),
    ));

    // The view resizes the Slint window to the box it was given, in logical
    // pixels, so the component sees its own width.
    expect(app['known-width'], 300);

    await tester.tap(find.byType(SlintView));
    await tester.pump();
    expect(app['clicks'], 1);
  });
}
