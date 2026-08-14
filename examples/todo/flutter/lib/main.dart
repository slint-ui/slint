// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:slint_flutter/slint_flutter.dart';
import 'package:todo/ui/todo.slint.dart';

/// The `.slint` file this example shares with its Rust, C++ and Node.js
/// siblings, loaded from the Flutter asset bundle.
const todoUiAsset = 'lib/ui/todo.slint';

TodoItem todo(String title, {bool checked = false}) =>
    TodoItem(title: title, checked: checked);

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  final source = await rootBundle.loadString(todoUiAsset);
  runApp(TodoApp(source: source));
}

class TodoApp extends StatelessWidget {
  const TodoApp({super.key, required this.source});

  final String source;

  @override
  Widget build(BuildContext context) => WidgetsApp(
        color: const Color(0xff000000),
        pageRouteBuilder: _defaultRouteBuilder,
        title: 'Slint todo',
        home: SlintView(load: () => buildTodoUi(source)),
      );

  /// The route builder `WidgetsApp` needs to turn `home` into a page route.
  static PageRoute<T> _defaultRouteBuilder<T>(
          RouteSettings settings, WidgetBuilder builder) =>
      PageRouteBuilder<T>(
        settings: settings,
        pageBuilder: (context, animation, secondaryAnimation) =>
            builder(context),
      );
}

MainWindow buildTodoUi(String source) {
  final app = MainWindow.loadSource(source, path: todoUiAsset);

  // Dart owns the list; the `todo-model` property is the view of it. Every
  // mutation writes the whole list back, which keeps the sorting and filtering
  // below down to ordinary list operations.
  final items = [
    todo('Implement the .slint file', checked: true),
    todo('Do the Rust part', checked: true),
    todo('Make the C++ code'),
    todo('Write some JavaScript code'),
    todo('Write the Dart part'),
    todo('Test the application'),
    todo('Ship to customer'),
    todo('???'),
    todo('Profit'),
  ];

  void refresh() {
    // Pick up the checkboxes the user toggled in the UI before rewriting the
    // model, otherwise sorting or filtering would discard them.
    for (final row in app.todoModel) {
      final index = items.indexWhere((item) => item.title == row.title);
      items[index] = todo(row.title, checked: row.checked);
    }

    final visible = items.toList();
    if (app.hideDoneItems) {
      visible.removeWhere((item) => item.checked);
    }
    if (app.isSortByName) {
      visible.sort(
        (a, b) => a.title.toLowerCase().compareTo(b.title.toLowerCase()),
      );
    }
    app.todoModel = visible;
  }

  app.onTodoAdded((title) {
    items.add(todo(title));
    refresh();
  });

  app.onRemoveDone(() {
    refresh();
    items.removeWhere((item) => item.checked);
    refresh();
  });

  app.onApplySortingAndFiltering(refresh);

  app.onPopupConfirmed(() {
    exit(0);
  });

  app.showHeader = true;
  app.todoModel = items;
  return app;
}
