// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

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
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

import 'src/diagnostics.dart';
import 'src/ffi.dart';

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
// Callback dispatch
//
// Slint invokes callbacks from inside the event loop, on the thread that
// started it — the main isolate's thread. That is exactly what
// `NativeCallable.isolateLocal` supports, so one dispatcher per process is
// enough; the `user_data` pointer carries the id of the Dart handler to run.
// ---------------------------------------------------------------------------

final Map<int, SlintCallback> _handlers = {};
final Map<int, void Function()> _timerHandlers = {};
int _nextHandlerId = 1;

Pointer<Utf8> _dispatchCallback(Pointer<Void> userData, Pointer<Utf8> argsJson) {
  final handler = _handlers[userData.address];
  if (handler == null) return nullptr;
  try {
    final args = jsonDecode(argsJson.toDartString()) as List<Object?>;
    final result = handler(args);
    // A null result means "void". Anything else goes back as JSON in a buffer
    // the Rust side hands straight to `_freeCallbackResult`.
    if (result == null) return nullptr;
    return jsonEncode(result).toNativeUtf8();
  } on Object catch (error, stack) {
    // An exception escaping the handler would surface as an uncaught error in
    // the isolate after the FFI trampoline has already returned, leaving the
    // Rust side to free an undefined return pointer. Report it here and
    // answer "no result" instead.
    stderr.writeln('Slint callback failed: $error\n$stack');
    return nullptr;
  }
}

void _freeCallbackResult(Pointer<Utf8> result) => malloc.free(result);

void _dispatchTimer(Pointer<Void> userData) {
  final handler = _timerHandlers[userData.address];
  if (handler == null) return;
  try {
    handler();
  } on Object catch (error, stack) {
    stderr.writeln('Slint timer callback failed: $error\n$stack');
  }
}

final _callbackDispatcher = NativeCallable<
        Pointer<Utf8> Function(Pointer<Void>, Pointer<Utf8>)>.isolateLocal(
    _dispatchCallback)
  // Without this the pending native callable keeps `dart run` alive forever
  // after the event loop has returned.
  ..keepIsolateAlive = false;

final _callbackResultFree =
    NativeCallable<Void Function(Pointer<Utf8>)>.isolateLocal(
        _freeCallbackResult)
      ..keepIsolateAlive = false;

final _timerDispatcher =
    NativeCallable<Void Function(Pointer<Void>)>.isolateLocal(_dispatchTimer)
      ..keepIsolateAlive = false;

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
}) {
  final ffi = SlintFfi.instance;
  return _instantiate(
    (compiler) => withNativeString(path, (p) => ffi.buildFromPath(compiler, p)),
    component: component,
    style: style,
    includePaths: includePaths,
  );
}

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
}) {
  final ffi = SlintFfi.instance;
  return _instantiate(
    (compiler) => withNativeString(
      source,
      (s) => withNativeString(
        path,
        (p) => ffi.buildFromSource(compiler, s, p),
      ),
    ),
    component: component,
    style: style,
    includePaths: includePaths,
  );
}

ComponentInstance _instantiate(
  Pointer<SlintCompilationResult> Function(Pointer<SlintCompiler>) build, {
  required String? component,
  required String? style,
  required List<String> includePaths,
}) {
  final ffi = SlintFfi.instance;
  final compiler = ffi.compilerNew();
  try {
    if (style != null) {
      withNativeString(style, (s) => ffi.compilerSetStyle(compiler, s));
    }
    for (final path in includePaths) {
      withNativeString(path, (p) => ffi.compilerAddIncludePath(compiler, p));
    }

    final result = build(compiler);
    if (result == nullptr) {
      throw SlintException('the Slint compiler crashed; see stderr for details');
    }
    try {
      if (ffi.resultHasErrors(result)) {
        final diagnostics = (takeEnvelope(ffi.resultDiagnostics(result))!
                as List<Object?>)
            .map((d) => Diagnostic.fromJson(d! as Map<String, dynamic>))
            .toList();
        throw SlintException('compilation failed', diagnostics);
      }

      final definition =
          withNativeString(component, (n) => ffi.resultComponent(result, n));
      if (definition == nullptr) {
        final names = takeEnvelope(ffi.resultComponentNames(result))!;
        throw SlintException(
          component == null
              ? 'the file exports no instantiable component'
              : 'no component named "$component"; the file exports $names',
        );
      }
      try {
        return ComponentInstance._create(definition);
      } finally {
        ffi.definitionFree(definition);
      }
    } finally {
      ffi.resultFree(result);
    }
  } finally {
    ffi.compilerFree(compiler);
  }
}

// ---------------------------------------------------------------------------
// Component instance
// ---------------------------------------------------------------------------

/// A live instance of a Slint component: one window and its properties,
/// callbacks, and global singletons.
class ComponentInstance implements SlintComponent {
  ComponentInstance._(this._handle);

  factory ComponentInstance._create(Pointer<SlintComponentDefinition> definition) {
    final ffi = SlintFfi.instance;
    final error = malloc<Pointer<Utf8>>()..value = nullptr;
    try {
      final handle = ffi.definitionCreate(definition, error);
      if (handle == nullptr) {
        throw SlintException(takeString(error.value));
      }
      return ComponentInstance._(handle);
    } finally {
      malloc.free(error);
    }
  }

  Pointer<SlintComponentInstance> _handle;
  final Set<int> _handlerIds = {};

  /// This runtime instance itself.
  @override
  ComponentInstance get instance => this;

  Pointer<SlintComponentInstance> get _live {
    if (_handle == nullptr) {
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
  void show() =>
      takeEnvelope(SlintFfi.instance.instanceShow(_live, true));

  void hide() => takeEnvelope(SlintFfi.instance.instanceShow(_live, false));

  /// Show the window and run the event loop until the last window closes or
  /// [quitEventLoop] is called.
  void run() => takeEnvelope(SlintFfi.instance.instanceRun(_live));

  /// Release the instance and its callback handlers. Using the instance
  /// afterwards throws [StateError].
  void dispose() {
    if (_handle == nullptr) return;
    SlintFfi.instance.instanceFree(_handle);
    _handle = nullptr;
    _handlers.removeWhere((id, _) => _handlerIds.contains(id));
    _handlerIds.clear();
  }

  Object? _getProperty(String? global, String name) {
    final ffi = SlintFfi.instance;
    return takeEnvelope(withNativeString(
      global,
      (g) => withNativeString(name, (n) => ffi.getProperty(_live, g, n)),
    ));
  }

  void _setProperty(String? global, String name, Object? value) {
    final ffi = SlintFfi.instance;
    takeEnvelope(withNativeString(
      global,
      (g) => withNativeString(
        name,
        (n) => withNativeString(
          jsonEncode(value),
          (v) => ffi.setProperty(_live, g, n, v),
        ),
      ),
    ));
  }

  Object? _invoke(String? global, String name, List<Object?> args) {
    final ffi = SlintFfi.instance;
    return takeEnvelope(withNativeString(
      global,
      (g) => withNativeString(
        name,
        (n) => withNativeString(
          jsonEncode(args),
          (a) => ffi.invoke(_live, g, n, a),
        ),
      ),
    ));
  }

  void _setCallback(String? global, String name, SlintCallback handler) {
    final ffi = SlintFfi.instance;
    final id = _nextHandlerId++;
    _handlers[id] = handler;
    _handlerIds.add(id);
    try {
      takeEnvelope(withNativeString(
        global,
        (g) => withNativeString(
          name,
          (n) => ffi.setCallback(
            _live,
            g,
            n,
            _callbackDispatcher.nativeFunction,
            _callbackResultFree.nativeFunction,
            Pointer<Void>.fromAddress(id),
          ),
        ),
      ));
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
void runEventLoop() => takeEnvelope(SlintFfi.instance.runEventLoop());

/// Make the running event loop return.
void quitEventLoop() => SlintFfi.instance.quitEventLoop();

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
    _timerHandlers[_id] = callback;
    _handle = SlintFfi.instance.timerStart(
      repeated,
      interval.inMilliseconds,
      _timerDispatcher.nativeFunction,
      Pointer<Void>.fromAddress(_id),
    );
  }

  final int _id;
  late Pointer<SlintTimerHandle> _handle;

  /// Stop the timer and release it. Calling this twice is harmless.
  void stop() {
    if (_handle == nullptr) return;
    SlintFfi.instance.timerFree(_handle);
    _handle = nullptr;
    _timerHandlers.remove(_id);
  }
}
