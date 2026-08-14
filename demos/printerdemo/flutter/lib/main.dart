// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import 'dart:async';
import 'dart:io';

import 'package:flutter/widgets.dart';
import 'package:slint_flutter/slint_flutter.dart';

/// The `.slint` files this demo shares with its Rust, C++, Node.js and Python
/// siblings, resolved relative to the project so `flutter run` finds them from
/// any working directory.
final printerdemoUi = '${Directory.current.path}/../ui/printerdemo.slint';

void main() => runApp(const PrinterDemo());

class PrinterDemo extends StatelessWidget {
  const PrinterDemo({super.key});

  @override
  Widget build(BuildContext context) => WidgetsApp(
        color: const Color(0xff000000),
        pageRouteBuilder: _defaultRouteBuilder,
        title: 'Slint printer demo',
        home: SlintView(load: PrinterQueue.load),
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

/// The print queue, and the logic that moves jobs through it.
///
/// The queue lives here in Dart; `PrinterQueue.printer-queue` in the `.slint`
/// file is the view of it. Writing the whole list back on every change is
/// cheap at this size and keeps the operations to plain list manipulation.
class PrinterQueue {
  PrinterQueue._(this._app) : _queue = _readInitialQueue(_app);

  final ComponentInstance _app;
  final List<Map<String, Object?>> _queue;

  static ComponentInstance load() {
    final app = loadFile(printerdemoUi);
    PrinterQueue._(app)._wire();
    return app;
  }

  /// The `.slint` file ships mock jobs so the design looks right in the
  /// viewer. Copy them out as the starting point, the way the other language
  /// versions of this demo do.
  static List<Map<String, Object?>> _readInitialQueue(ComponentInstance app) =>
      (app.global('PrinterQueue')['printer-queue']! as List<Object?>)
          .cast<Map<String, Object?>>()
          .map(Map<String, Object?>.from)
          .toList();

  void _wire() {
    _app.global('PrinterState')['ink-levels'] = const [
      {'color': '#00ffff', 'level': 0.40},
      {'color': '#ffff00', 'level': 0.20},
      {'color': '#ff00ff', 'level': 0.50},
      {'color': '#000000', 'level': 0.80},
    ];

    _app.global('PrinterQueue')
      ..setCallback('start-job', (args) {
        _queue.add({
          'status': 'waiting',
          'progress': 0,
          'title': args[0]! as String,
          'owner': 'me@example.com',
          'pages': 1,
          'size': '100kB',
          'submission-date': _now(),
        });
        _publish();
        return null;
      })
      ..setCallback('cancel-job', (args) {
        final index = (args[0]! as num).toInt();
        if (index >= 0 && index < _queue.length) {
          _queue.removeAt(index);
          _publish();
        }
        return null;
      })
      ..setCallback('pause-job', (args) {
        final index = (args[0]! as num).toInt();
        if (index >= 0 && index < _queue.length) {
          final job = _queue[index];
          job['status'] = job['status'] == 'printing' ? 'waiting' : 'printing';
          _publish();
        }
        return null;
      });

    _app.setCallback('quit', (_) {
      exit(0);
    });

    // Flutter keeps its own event loop running, so an ordinary Dart timer is
    // enough here. A pure Dart application, where `run()` owns the thread,
    // would use `SlintTimer` instead.
    Timer.periodic(const Duration(seconds: 1), (_) => _advance());
    _publish();
  }

  /// Move the job at the head of the queue one percent further along, and
  /// retire it once it is done.
  void _advance() {
    if (_queue.isEmpty) return;
    final head = _queue.first;
    head['status'] = 'printing';
    final progress = (head['progress']! as num).toInt() + 1;
    if (progress >= 100) {
      _queue.removeAt(0);
    } else {
      head['progress'] = progress;
    }
    _publish();
  }

  void _publish() =>
      _app.global('PrinterQueue')['printer-queue'] = _queue;

  static String _now() {
    final now = DateTime.now();
    String two(int v) => v.toString().padLeft(2, '0');
    return '${two(now.hour)}:${two(now.minute)} '
        '${two(now.day)}/${two(now.month)}/${two(now.year % 100)}';
  }
}
