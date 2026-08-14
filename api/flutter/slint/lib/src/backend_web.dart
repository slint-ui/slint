// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/// The [SlintBackend] for the web, where `slint_dart` is a WebAssembly module.
///
/// The module is the one `wasm-pack` builds out of `api/flutter/rust`: its
/// `wasm.rs` exposes the same entry points the C ABI has, taking strings and
/// numbers so JavaScript — and therefore Dart — can call them without touching
/// wasm memory. Handles are wasm addresses and mean nothing outside the module.
///
/// Loading is asynchronous, which is why `initSlint()` exists at all: the
/// module has to be fetched and instantiated before the first call.
library;

import 'dart:js_interop';
import 'dart:typed_data';

import 'backend.dart';
import 'diagnostics.dart';

SlintBackend createBackend() => WebBackend();

/// The `slint_dart.js` module `wasm-pack build --target web` writes.
///
/// The names are the ones `wasm.rs` exports; `@JS` keeps them out of the Dart
/// side of the fence.
extension type _SlintModule(JSObject _) implements JSObject {
  /// The module's default export: it fetches and instantiates the wasm.
  /// `default` is a Dart keyword, hence the rename.
  @JS('default')
  external JSPromise<JSAny?> init();

  @JS('compiler_new')
  external int compilerNew();
  @JS('compiler_free')
  external void compilerFree(int compiler);
  @JS('compiler_set_style')
  external void compilerSetStyle(int compiler, String style);
  @JS('compiler_add_include_path')
  external void compilerAddIncludePath(int compiler, String path);
  @JS('build_from_source')
  external int buildFromSource(int compiler, String source, String path);

  @JS('result_free')
  external void resultFree(int result);
  @JS('result_has_errors')
  external bool resultHasErrors(int result);
  @JS('result_diagnostics')
  external String resultDiagnostics(int result);
  @JS('result_component_names')
  external String resultComponentNames(int result);
  @JS('result_component')
  external int resultComponent(int result, String? name);

  @JS('definition_free')
  external void definitionFree(int definition);
  @JS('definition_create')
  external String definitionCreate(int definition);

  @JS('instance_free')
  external void instanceFree(int instance);
  @JS('instance_get_property')
  external String getProperty(int instance, String? global, String name);
  @JS('instance_set_property')
  external String setProperty(
    int instance,
    String? global,
    String name,
    String json,
  );
  @JS('instance_invoke')
  external String invoke(
    int instance,
    String? global,
    String name,
    String argsJson,
  );
  @JS('instance_set_callback')
  external String setCallback(
    int instance,
    String? global,
    String name,
    int id,
  );
  @JS('instance_show')
  external String instanceShow(int instance, bool visible);

  @JS('set_callback_dispatcher')
  external void setCallbackDispatcher(JSFunction dispatcher);
  @JS('set_timer_dispatcher')
  external void setTimerDispatcher(JSFunction dispatcher);

  @JS('timer_start')
  external int timerStart(bool repeated, double intervalMs, int id);
  @JS('timer_free')
  external void timerFree(int timer);

  @JS('embedded_init')
  external String embeddedInit();
  @JS('embedded_resize')
  external String embeddedResize(int width, int height, double scaleFactor);
  @JS('embedded_render')
  external JSUint8Array? embeddedRender(int width, int height);
  @JS('embedded_tick')
  external double embeddedTick();
  @JS('embedded_has_active_animations')
  external bool embeddedHasActiveAnimations();
  @JS('embedded_pointer_event')
  external String embeddedPointerEvent(
    int kind,
    double x,
    double y,
    int button,
    double deltaX,
    double deltaY,
  );
  @JS('embedded_key_event')
  external String embeddedKeyEvent(int kind, String text);
  @JS('embedded_focus_event')
  external String embeddedFocusEvent(bool focused);
}

@JS('console.error')
external void _consoleError(String message);

class WebBackend implements SlintBackend {
  _SlintModule? _module;

  CallbackDispatch? _onCallback;
  TimerDispatch? _onTimer;

  _SlintModule get _slint {
    final module = _module;
    if (module == null) {
      throw StateError(
        'the Slint WebAssembly module is not loaded yet; '
        'await initSlint() before using Slint on the web',
      );
    }
    return module;
  }

  @override
  Future<void> initialize({String? scriptUrl}) async {
    if (_module != null) return;
    final module = _SlintModule(
      (await importModule((scriptUrl ?? './slint_dart.js').toJS).toDart),
    );
    // The default export instantiates the wasm alongside the module.
    await module.init().toDart;
    _module = module;
    _installDispatchers();
  }

  @override
  void installDispatchers(CallbackDispatch onCallback, TimerDispatch onTimer) {
    _onCallback = onCallback;
    _onTimer = onTimer;
    if (_module != null) _installDispatchers();
  }

  void _installDispatchers() {
    final module = _module;
    if (module == null || _onCallback == null) return;
    module.setCallbackDispatcher(
      ((JSNumber id, JSString argsJson) {
        final result = _onCallback!(id.toDartInt, argsJson.toDart);
        return result?.toJS;
      }).toJS,
    );
    module.setTimerDispatcher(
      ((JSNumber id) => _onTimer?.call(id.toDartInt)).toJS,
    );
  }

  @override
  void reportError(String message) => _consoleError(message);

  // Compiler ---------------------------------------------------------------

  @override
  int compilerNew() => _slint.compilerNew();

  @override
  void compilerFree(int compiler) => _slint.compilerFree(compiler);

  @override
  void compilerSetStyle(int compiler, String style) =>
      _slint.compilerSetStyle(compiler, style);

  @override
  void compilerAddIncludePath(int compiler, String path) =>
      _slint.compilerAddIncludePath(compiler, path);

  @override
  int buildFromPath(int compiler, String path) => throw SlintException(
        'loadFile is not available on the web: a browser has no filesystem. '
        'Fetch the .slint source and use loadSource instead.',
      );

  @override
  int buildFromSource(int compiler, String source, String path) =>
      _slint.buildFromSource(compiler, source, path);

  // Compilation result -----------------------------------------------------

  @override
  void resultFree(int result) => _slint.resultFree(result);

  @override
  bool resultHasErrors(int result) => _slint.resultHasErrors(result);

  @override
  Object? resultDiagnostics(int result) =>
      decodeEnvelope(_slint.resultDiagnostics(result));

  @override
  Object? resultComponentNames(int result) =>
      decodeEnvelope(_slint.resultComponentNames(result));

  @override
  int resultComponent(int result, String? name) =>
      _slint.resultComponent(result, name);

  // Component definition ---------------------------------------------------

  @override
  void definitionFree(int definition) => _slint.definitionFree(definition);

  @override
  int definitionCreate(int definition) =>
      decodeEnvelope(_slint.definitionCreate(definition))! as int;

  // Component instance -----------------------------------------------------

  @override
  void instanceFree(int instance) => _slint.instanceFree(instance);

  @override
  Object? getProperty(int instance, String? global, String name) =>
      decodeEnvelope(_slint.getProperty(instance, global, name));

  @override
  void setProperty(int instance, String? global, String name, String json) =>
      decodeEnvelope(_slint.setProperty(instance, global, name, json));

  @override
  Object? invoke(int instance, String? global, String name, String argsJson) =>
      decodeEnvelope(_slint.invoke(instance, global, name, argsJson));

  @override
  void setCallback(int instance, String? global, String name, int id) =>
      decodeEnvelope(_slint.setCallback(instance, global, name, id));

  @override
  void instanceShow(int instance, bool visible) =>
      decodeEnvelope(_slint.instanceShow(instance, visible));

  // Event loop -------------------------------------------------------------

  @override
  void instanceRun(int instance) => throw SlintException(
        'run() is not available on the web: the browser owns the event loop. '
        'Render through SlintView, or drive SlintSurface yourself.',
      );

  @override
  void runEventLoop() => throw SlintException(
        'runEventLoop() is not available on the web: the browser owns the '
        'event loop.',
      );

  @override
  void quitEventLoop() {}

  // Timers -----------------------------------------------------------------

  @override
  int timerStart(bool repeated, int intervalMs, int id) =>
      _slint.timerStart(repeated, intervalMs.toDouble(), id);

  @override
  void timerFree(int timer) => _slint.timerFree(timer);

  // Embedded rendering -----------------------------------------------------

  @override
  void embeddedInit() => decodeEnvelope(_slint.embeddedInit());

  @override
  void embeddedResize(int width, int height, double scaleFactor) =>
      decodeEnvelope(_slint.embeddedResize(width, height, scaleFactor));

  @override
  Uint8List? embeddedRender(int width, int height) =>
      _slint.embeddedRender(width, height)?.toDart;

  @override
  int embeddedTick() => _slint.embeddedTick().toInt();

  @override
  bool embeddedHasActiveAnimations() => _slint.embeddedHasActiveAnimations();

  @override
  void embeddedPointerEvent(
    int kind,
    double x,
    double y,
    int button,
    double deltaX,
    double deltaY,
  ) =>
      decodeEnvelope(
        _slint.embeddedPointerEvent(kind, x, y, button, deltaX, deltaY),
      );

  @override
  void embeddedKeyEvent(int kind, String text) =>
      decodeEnvelope(_slint.embeddedKeyEvent(kind, text));

  @override
  void embeddedFocusEvent(bool focused) =>
      decodeEnvelope(_slint.embeddedFocusEvent(focused));

  @override
  void embeddedDispose() {}
}
