// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/// Slint for Dart and Flutter.
///
/// Write the user interface in `.slint`, the logic in Dart:
///
/// ```dart
/// import 'package:slint/slint.dart';
///
/// void main() {
///   final app = loadFile('ui/todo.slint');
///   app['todo-model'] = [
///     {'title': 'Write the Dart part', 'checked': false},
///   ];
///   app.setCallback('todo-added', (args) {
///     final items = app['todo-model']! as List<Object?>;
///     app['todo-model'] = [...items, {'title': args[0], 'checked': false}];
///   });
///   app.run();
/// }
/// ```
///
/// Values cross the boundary as plain Dart data: `num`, `String`, `bool`,
/// `List`, and `Map<String, Object?>` for Slint structs. Colors and brushes are
/// CSS-style strings such as `'#00ffffff'`.
///
/// Everything in this library must be used from the main isolate, because
/// that is where the Slint event loop runs. That matches the constraint the
/// Python and Node.js bindings impose.
library;

import 'dart:convert';

import 'src/backend.dart';
import 'src/diagnostics.dart';

export 'src/diagnostics.dart' show Diagnostic, SlintException;
export 'src/embedded.dart'
    show KeyEventKind, PointerButton, PointerEventKind, SlintKey, SlintSurface;

/// A handler for a Slint callback. It receives the callback arguments and
/// returns the callback's result, or null for a callback that returns nothing.
typedef SlintCallback = Object? Function(List<Object?> args);

/// A Slint component that can provide its underlying runtime instance.
///
/// Generated component wrappers implement this interface so that APIs such as
/// `SlintView` can accept both typed wrappers and a plain [ComponentInstance].
abstract interface class SlintComponent {
  /// The live runtime instance represented by this component.
  ComponentInstance get instance;
}

// ---------------------------------------------------------------------------
// Loading the library
// ---------------------------------------------------------------------------

/// Make Slint usable.
///
/// On the web this fetches and instantiates the WebAssembly module, which is
/// asynchronous, so `await` this before the first [loadSource]. [scriptUrl]
/// points at the `slint_dart.js` that `wasm-pack` produced. It is a module
/// specifier, so a relative one needs the leading `./`; the default expects
/// the file beside `index.html`.
///
/// On every other platform the library is loaded on first use and this returns
/// immediately, so calling it unconditionally is the portable thing to do.
Future<void> initSlint({String? scriptUrl}) =>
    backend.initialize(scriptUrl: scriptUrl);

// ---------------------------------------------------------------------------
// Callback dispatch
//
// Slint invokes callbacks from inside the event loop, on the thread that
// started it — the main isolate's thread. One dispatcher per process is
// enough, and the id of the Dart handler to run travels with the call.
// ---------------------------------------------------------------------------

final Map<int, SlintCallback> _handlers = {};
final Map<int, void Function()> _timerHandlers = {};
int _nextHandlerId = 1;

String? _dispatchCallback(int id, String argsJson) {
  final handler = _handlers[id];
  if (handler == null) return null;
  try {
    final result = handler(jsonDecode(argsJson) as List<Object?>);
    // A null result means "void".
    return result == null ? null : jsonEncode(result);
  } on Object catch (error, stack) {
    // An exception escaping the handler would surface as an uncaught error
    // after the call back into Dart has already returned, leaving the Rust
    // side with an undefined result. Report it here and answer "no result".
    backend.reportError('Slint callback failed: $error\n$stack');
    return null;
  }
}

void _dispatchTimer(int id) {
  final handler = _timerHandlers[id];
  if (handler == null) return;
  try {
    handler();
  } on Object catch (error, stack) {
    backend.reportError('Slint timer callback failed: $error\n$stack');
  }
}

var _dispatchersInstalled = false;

void _installDispatchers() {
  if (_dispatchersInstalled) return;
  backend.installDispatchers(_dispatchCallback, _dispatchTimer);
  _dispatchersInstalled = true;
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Compile `.slint` [path] and instantiate one of its components.
///
/// [component] picks a component by name; by default the last exported one is
/// used. [style] selects a widget style (`fluent`, `material`, `cupertino`, …)
/// and [includePaths] adds directories for `import` statements to search.
///
/// Throws [SlintException] with the compiler [Diagnostic]s if the file does not
/// compile.
ComponentInstance loadFile(
  String path, {
  String? component,
  String? style,
  List<String> includePaths = const [],
}) =>
    _instantiate(
      (compiler) => backend.buildFromPath(compiler, path),
      component: component,
      style: style,
      includePaths: includePaths,
    );

/// Compile `.slint` [source] and instantiate one of its components.
///
/// [path] is only used for diagnostics and to resolve relative imports.
/// See [loadFile] for the remaining arguments.
ComponentInstance loadSource(
  String source, {
  String path = 'source.slint',
  String? component,
  String? style,
  List<String> includePaths = const [],
}) =>
    _instantiate(
      (compiler) => backend.buildFromSource(compiler, source, path),
      component: component,
      style: style,
      includePaths: includePaths,
    );

ComponentInstance _instantiate(
  int Function(int compiler) build, {
  required String? component,
  required String? style,
  required List<String> includePaths,
}) {
  final compiler = backend.compilerNew();
  try {
    if (style != null) backend.compilerSetStyle(compiler, style);
    for (final path in includePaths) {
      backend.compilerAddIncludePath(compiler, path);
    }

    final result = build(compiler);
    if (result == 0) {
      throw SlintException(
          'the Slint compiler crashed; see stderr for details');
    }
    try {
      if (backend.resultHasErrors(result)) {
        final diagnostics =
            (backend.resultDiagnostics(result)! as List<Object?>)
                .map((d) => Diagnostic.fromJson(d! as Map<String, dynamic>))
                .toList();
        throw SlintException('compilation failed', diagnostics);
      }

      final definition = backend.resultComponent(result, component);
      if (definition == 0) {
        final names = backend.resultComponentNames(result)!;
        throw SlintException(
          component == null
              ? 'the file exports no instantiable component'
              : 'no component named "$component"; the file exports $names',
        );
      }
      try {
        return ComponentInstance._(backend.definitionCreate(definition));
      } finally {
        backend.definitionFree(definition);
      }
    } finally {
      backend.resultFree(result);
    }
  } finally {
    backend.compilerFree(compiler);
  }
}

// ---------------------------------------------------------------------------
// Component instance
// ---------------------------------------------------------------------------

/// A live instance of a Slint component: one window and its properties,
/// callbacks, and global singletons.
class ComponentInstance implements SlintComponent {
  ComponentInstance._(this._handle);

  int _handle;
  final Set<int> _handlerIds = {};

  /// This runtime instance itself.
  @override
  ComponentInstance get instance => this;

  int get _live {
    if (_handle == 0) {
      throw StateError('this ComponentInstance has been disposed');
    }
    return _handle;
  }

  /// Read a property by its Slint name.
  Object? getProperty(String name) => _getProperty(null, name);

  /// Write a property by its Slint name.
  void setProperty(String name, Object? value) =>
      _setProperty(null, name, value);

  /// Read a property, for example `app['todo-model']`.
  Object? operator [](String name) => getProperty(name);

  /// Write a property, for example `app['title'] = 'Hello'`.
  void operator []=(String name, Object? value) => setProperty(name, value);

  /// Call a callback or a public function declared on the component.
  Object? invoke(String name, [List<Object?> args = const []]) =>
      _invoke(null, name, args);

  /// Handle a callback declared on the component.
  ///
  /// Replaces any handler set earlier for the same callback.
  void setCallback(String name, SlintCallback handler) =>
      _setCallback(null, name, handler);

  /// Reach into an exported global singleton.
  SlintGlobal global(String name) => SlintGlobal._(this, name);

  /// Make the window visible without running the event loop. Use [run] unless
  /// you drive the event loop yourself.
  void show() => backend.instanceShow(_live, true);

  void hide() => backend.instanceShow(_live, false);

  /// Show the window and run the event loop until the last window closes or
  /// [quitEventLoop] is called.
  void run() => backend.instanceRun(_live);

  /// Release the instance and its callback handlers. Using the instance
  /// afterwards throws [StateError].
  void dispose() {
    if (_handle == 0) return;
    backend.instanceFree(_handle);
    _handle = 0;
    _handlers.removeWhere((id, _) => _handlerIds.contains(id));
    _handlerIds.clear();
  }

  Object? _getProperty(String? global, String name) =>
      backend.getProperty(_live, global, name);

  void _setProperty(String? global, String name, Object? value) =>
      backend.setProperty(_live, global, name, jsonEncode(value));

  Object? _invoke(String? global, String name, List<Object?> args) =>
      backend.invoke(_live, global, name, jsonEncode(args));

  void _setCallback(String? global, String name, SlintCallback handler) {
    _installDispatchers();
    final id = _nextHandlerId++;
    _handlers[id] = handler;
    _handlerIds.add(id);
    try {
      backend.setCallback(_live, global, name, id);
    } on Object {
      _handlers.remove(id);
      _handlerIds.remove(id);
      rethrow;
    }
  }
}

/// A global singleton of a component, obtained with
/// [ComponentInstance.global]. It has the same property, callback, and function
/// access as the component itself.
class SlintGlobal {
  SlintGlobal._(this._instance, this.name);

  final ComponentInstance _instance;

  /// The exported name of the singleton.
  final String name;

  /// Read a property by its Slint name.
  Object? getProperty(String property) =>
      _instance._getProperty(name, property);

  /// Write a property by its Slint name.
  void setProperty(String property, Object? value) =>
      _instance._setProperty(name, property, value);

  Object? operator [](String property) => getProperty(property);

  void operator []=(String property, Object? value) =>
      setProperty(property, value);

  Object? invoke(String function, [List<Object?> args = const []]) =>
      _instance._invoke(name, function, args);

  void setCallback(String callback, SlintCallback handler) =>
      _instance._setCallback(name, callback, handler);
}

// ---------------------------------------------------------------------------
// Event loop and timers
// ---------------------------------------------------------------------------

/// Run the Slint event loop. [ComponentInstance.run] is usually more
/// convenient; use this when you showed several windows yourself.
void runEventLoop() => backend.runEventLoop();

/// Make the running event loop return.
void quitEventLoop() => backend.quitEventLoop();

/// A timer driven by the Slint event loop.
///
/// Dart's own `Timer` never fires while [ComponentInstance.run] owns the
/// thread, so use this one for anything periodic in a Slint application.
class SlintTimer {
  /// Call [callback] every [interval] until [stop].
  SlintTimer.periodic(Duration interval, void Function() callback)
      : this._(true, interval, callback);

  /// Call [callback] once, after [delay].
  SlintTimer.single(Duration delay, void Function() callback)
      : this._(false, delay, callback);

  SlintTimer._(bool repeated, Duration interval, void Function() callback)
      : _id = _nextHandlerId++ {
    _installDispatchers();
    _timerHandlers[_id] = callback;
    _handle = backend.timerStart(repeated, interval.inMilliseconds, _id);
  }

  final int _id;
  late int _handle;

  /// Stop the timer and release it. Calling this twice is harmless.
  void stop() {
    if (_handle == 0) return;
    backend.timerFree(_handle);
    _handle = 0;
    _timerHandlers.remove(_id);
  }
}
