// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:todo/main.dart' as app;
import 'package:todo/ui/todo.slint.dart';

/// The route builder `WidgetsApp` needs to turn `home` into a page route.
PageRoute<T> _defaultRouteBuilder<T>(
        RouteSettings settings, WidgetBuilder builder) =>
    PageRouteBuilder<T>(
      settings: settings,
      pageBuilder: (context, animation, secondaryAnimation) => builder(context),
    );

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  // Only one SlintView can be alive in an isolate, so this test pumps the
  // widget once and exercises `buildTodoUi` through the loaded window.
  testWidgets('SlintView renders todo UI and buildTodoUi keeps the model in sync',
      (tester) async {
    final source = await rootBundle.loadString(app.todoUiAsset);
    late MainWindow window;

    await tester.pumpWidget(WidgetsApp(
      color: const Color(0xff000000),
      pageRouteBuilder: _defaultRouteBuilder,
      home: SizedBox(
        width: 400,
        height: 600,
        child: SlintView(
          key: const ValueKey('todo-slint-view'),
          load: () => window = app.buildTodoUi(source),
        ),
      ),
    ));
    await tester.pump();

    expect(find.byKey(const ValueKey('todo-slint-view')), findsOneWidget);
    expect(window.showHeader, isTrue);
    expect(window.todoModel.length, 9);
    expect(
      window.todoModel.map((item) => item.title),
      contains('Write the Dart part'),
    );

    window.invokeTodoAdded('Buy milk');
    expect(window.todoModel.length, 10);
    expect(
      window.todoModel.any((item) => item.title == 'Buy milk'),
      isTrue,
    );

    window.isSortByName = true;
    window.invokeApplySortingAndFiltering();
    final titles = window.todoModel.map((item) => item.title).toList();
    final sorted = List<String>.from(titles)
      ..sort((a, b) => a.toLowerCase().compareTo(b.toLowerCase()));
    expect(titles, sorted);

    window.hideDoneItems = true;
    window.invokeApplySortingAndFiltering();
    expect(window.todoModel.every((item) => !item.checked), isTrue);
    expect(window.todoModel.length, 8);

    final target = window.todoModel.firstWhere((item) => !item.checked);
    window.todoModel = window.todoModel
        .map((item) => item.title == target.title
            ? app.todo(target.title, checked: true)
            : item)
        .toList();
    window.hideDoneItems = false;
    window.invokeApplySortingAndFiltering();
    expect(
      window.todoModel.any(
        (item) => item.title == target.title && item.checked,
      ),
      isTrue,
    );
  });
}
