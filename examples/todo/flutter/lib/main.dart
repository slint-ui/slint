// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:slint_flutter/slint_flutter.dart';

/// The `.slint` file this example shares with its Rust, C++ and Node.js
/// siblings, resolved relative to the project so `flutter run` finds it from
/// any working directory.
final todoUi = '${Directory.current.path}/../ui/todo.slint';

/// One row of the list. It matches the `TodoItem` struct in `todo.slint`;
/// Slint structs cross the boundary as plain maps.
Map<String, Object?> todo(String title, {bool checked = false}) =>
    {'title': title, 'checked': checked};

void main() => runApp(const TodoApp());

class TodoApp extends StatelessWidget {
  const TodoApp({super.key});

  @override
  Widget build(BuildContext context) => MaterialApp(
        title: 'Slint todo',
        home: Scaffold(body: SlintView(load: buildTodoUi)),
      );
}

ComponentInstance buildTodoUi() {
  final app = loadFile(todoUi);

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
    final shown = app['todo-model']! as List<Object?>;
    for (final row in shown.cast<Map<String, Object?>>()) {
      items.firstWhere((item) => item['title'] == row['title'])['checked'] =
          row['checked'];
    }

    final visible = items.toList();
    if (app['hide-done-items']! as bool) {
      visible.removeWhere((item) => item['checked']! as bool);
    }
    if (app['is-sort-by-name']! as bool) {
      visible.sort((a, b) => (a['title']! as String)
          .toLowerCase()
          .compareTo((b['title']! as String).toLowerCase()));
    }
    app['todo-model'] = visible;
  }

  app.setCallback('todo-added', (args) {
    items.add(todo(args[0]! as String));
    refresh();
    return null;
  });

  app.setCallback('remove-done', (_) {
    refresh();
    items.removeWhere((item) => item['checked']! as bool);
    refresh();
    return null;
  });

  app.setCallback('apply_sorting_and_filtering', (_) {
    refresh();
    return null;
  });

  app.setCallback('popup_confirmed', (_) {
    exit(0);
  });

  app['show-header'] = true;
  app['todo-model'] = items;
  return app;
}
