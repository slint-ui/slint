// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import 'package:flutter/widgets.dart';
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

final class _GeneratedApp implements SlintComponent {
  _GeneratedApp(this.instance);

  @override
  final ComponentInstance instance;

  int get clicks => instance.getProperty('clicks')! as int;
}

/// The route builder `WidgetsApp` needs to turn `home` into a page route.
PageRoute<T> _defaultRouteBuilder<T>(
        RouteSettings settings, WidgetBuilder builder) =>
    PageRouteBuilder<T>(
      settings: settings,
      pageBuilder: (context, animation, secondaryAnimation) => builder(context),
    );

void main() {
  testWidgets('sizes the surface and forwards taps to Slint', (tester) async {
    late ComponentInstance app;
    ComponentInstance loadApp() => app = loadSource(_app);
    final view = SlintView(load: loadApp);
    final ComponentInstance Function() compatibleLoad = view.load;
    expect(identical(compatibleLoad, loadApp), isTrue);

    await tester.pumpWidget(WidgetsApp(
      color: const Color(0xff000000),
      pageRouteBuilder: _defaultRouteBuilder,
      home: Center(
        child: SizedBox(
          width: 300,
          height: 200,
          child: SlintView(
            key: const ValueKey('slint-view'),
            load: compatibleLoad,
          ),
        ),
      ),
    ));

    // The view resizes the Slint window to the box it was given, in logical
    // pixels, so the component sees its own width.
    expect(app['known-width'], 300);

    expect(find.byType(SlintView), findsOneWidget);
    await tester.tap(find.byKey(const ValueKey('slint-view')));
    await tester.pump();
    expect(app['clicks'], 1);
  });

  testWidgets('accepts a generated component wrapper', (tester) async {
    late _GeneratedApp app;

    await tester.pumpWidget(WidgetsApp(
      color: const Color(0xff000000),
      pageRouteBuilder: _defaultRouteBuilder,
      home: SizedBox(
        width: 300,
        height: 200,
        child: SlintView(
          key: const ValueKey('slint-view'),
          load: () => app = _GeneratedApp(loadSource(_app)),
        ),
      ),
    ));

    await tester.tap(find.byKey(const ValueKey('slint-view')));
    await tester.pump();
    expect(app.clicks, 1);
  });
}
