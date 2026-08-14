// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/// The [SlintBackend] for platforms with a shared library and `dart:ffi`.
///
/// It owns every pointer: handles leave as addresses, strings are copied in
/// and out around each call, and the frame buffer the software renderer draws
/// into lives here.
library;

import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import 'backend.dart';
import 'diagnostics.dart';
import 'ffi.dart';

SlintBackend createBackend() => NativeBackend();

class NativeBackend implements SlintBackend {
  SlintFfi get _ffi => SlintFfi.instance;

  static CallbackDispatch? _onCallback;
  static TimerDispatch? _onTimer;

  Pointer<Uint8> _frame = nullptr;
  int _framePixels = 0;

  @override
  Future<void> initialize({String? scriptUrl}) async {}

  @override
  void installDispatchers(CallbackDispatch onCallback, TimerDispatch onTimer) {
    _onCallback = onCallback;
    _onTimer = onTimer;
  }

  @override
  void reportError(String message) => stderr.writeln(message);

  // The trampolines Slint calls. They are static because
  // `NativeCallable.isolateLocal` needs a top-level target, and one dispatcher
  // per process is enough: Slint invokes callbacks on the thread that runs the
  // event loop, which is the main isolate's.
  static Pointer<Char> _dispatchCallback(
    Pointer<Void> userData,
    Pointer<Char> argsJson,
  ) {
    final result = _onCallback?.call(
      userData.address,
      argsJson.cast<Utf8>().toDartString(),
    );
    if (result == null) return nullptr;
    return result.toNativeUtf8().cast<Char>();
  }

  static void _freeCallbackResult(Pointer<Char> result) => malloc.free(result);

  static void _dispatchTimer(Pointer<Void> userData) =>
      _onTimer?.call(userData.address);

  // The signatures come from the generated bindings, so a change on the Rust
  // side stops compiling here instead of corrupting the stack at runtime.
  static final _callbackTrampoline =
      NativeCallable<DartCallbackFunction>.isolateLocal(_dispatchCallback)
        // Without this the pending native callable keeps `dart run` alive
        // forever after the event loop has returned.
        ..keepIsolateAlive = false;

  static final _freeTrampoline =
      NativeCallable<DartFreeFunction>.isolateLocal(_freeCallbackResult)
        ..keepIsolateAlive = false;

  static final _timerTrampoline =
      NativeCallable<Void Function(Pointer<Void>)>.isolateLocal(_dispatchTimer)
        ..keepIsolateAlive = false;

  // Compiler ---------------------------------------------------------------

  @override
  int compilerNew() => _ffi.compilerNew().address;

  @override
  void compilerFree(int compiler) =>
      _ffi.compilerFree(Pointer.fromAddress(compiler));

  @override
  void compilerSetStyle(int compiler, String style) => withNativeString(
        style,
        (s) => _ffi.compilerSetStyle(Pointer.fromAddress(compiler), s),
      );

  @override
  void compilerAddIncludePath(int compiler, String path) => withNativeString(
        path,
        (p) => _ffi.compilerAddIncludePath(Pointer.fromAddress(compiler), p),
      );

  @override
  int buildFromPath(int compiler, String path) => withNativeString(
        path,
        (p) => _ffi.buildFromPath(Pointer.fromAddress(compiler), p).address,
      );

  @override
  int buildFromSource(int compiler, String source, String path) =>
      withNativeString(
        source,
        (s) => withNativeString(
          path,
          (p) =>
              _ffi.buildFromSource(Pointer.fromAddress(compiler), s, p).address,
        ),
      );

  // Compilation result -----------------------------------------------------

  @override
  void resultFree(int result) => _ffi.resultFree(Pointer.fromAddress(result));

  @override
  bool resultHasErrors(int result) =>
      _ffi.resultHasErrors(Pointer.fromAddress(result));

  @override
  Object? resultDiagnostics(int result) =>
      takeEnvelope(_ffi.resultDiagnostics(Pointer.fromAddress(result)));

  @override
  Object? resultComponentNames(int result) =>
      takeEnvelope(_ffi.resultComponentNames(Pointer.fromAddress(result)));

  @override
  int resultComponent(int result, String? name) => withNativeString(
        name,
        (n) => _ffi.resultComponent(Pointer.fromAddress(result), n).address,
      );

  // Component definition ---------------------------------------------------

  @override
  void definitionFree(int definition) =>
      _ffi.definitionFree(Pointer.fromAddress(definition));

  @override
  int definitionCreate(int definition) {
    final error = malloc<Pointer<Char>>()..value = nullptr;
    try {
      final instance =
          _ffi.definitionCreate(Pointer.fromAddress(definition), error);
      if (instance == nullptr) throw SlintException(takeString(error.value));
      return instance.address;
    } finally {
      malloc.free(error);
    }
  }

  // Component instance -----------------------------------------------------

  @override
  void instanceFree(int instance) =>
      _ffi.instanceFree(Pointer.fromAddress(instance));

  @override
  Object? getProperty(int instance, String? global, String name) =>
      takeEnvelope(withNativeString(
        global,
        (g) => withNativeString(
          name,
          (n) => _ffi.getProperty(Pointer.fromAddress(instance), g, n),
        ),
      ));

  @override
  void setProperty(int instance, String? global, String name, String json) =>
      takeEnvelope(withNativeString(
        global,
        (g) => withNativeString(
          name,
          (n) => withNativeString(
            json,
            (v) => _ffi.setProperty(Pointer.fromAddress(instance), g, n, v),
          ),
        ),
      ));

  @override
  Object? invoke(int instance, String? global, String name, String argsJson) =>
      takeEnvelope(withNativeString(
        global,
        (g) => withNativeString(
          name,
          (n) => withNativeString(
            argsJson,
            (a) => _ffi.invoke(Pointer.fromAddress(instance), g, n, a),
          ),
        ),
      ));

  @override
  void setCallback(int instance, String? global, String name, int id) =>
      takeEnvelope(withNativeString(
        global,
        (g) => withNativeString(
          name,
          (n) => _ffi.setCallback(
            Pointer.fromAddress(instance),
            g,
            n,
            _callbackTrampoline.nativeFunction,
            _freeTrampoline.nativeFunction,
            Pointer<Void>.fromAddress(id),
          ),
        ),
      ));

  @override
  void instanceShow(int instance, bool visible) =>
      takeEnvelope(_ffi.instanceShow(Pointer.fromAddress(instance), visible));

  // Event loop -------------------------------------------------------------

  @override
  void instanceRun(int instance) =>
      takeEnvelope(_ffi.instanceRun(Pointer.fromAddress(instance)));

  @override
  void runEventLoop() => takeEnvelope(_ffi.runEventLoop());

  @override
  void quitEventLoop() => _ffi.quitEventLoop();

  // Timers -----------------------------------------------------------------

  @override
  int timerStart(bool repeated, int intervalMs, int id) => _ffi
      .timerStart(
        repeated,
        intervalMs,
        _timerTrampoline.nativeFunction,
        Pointer<Void>.fromAddress(id),
      )
      .address;

  @override
  void timerFree(int timer) => _ffi.timerFree(Pointer.fromAddress(timer));

  // Embedded rendering -----------------------------------------------------

  @override
  void embeddedInit() => takeEnvelope(_ffi.embeddedInit());

  @override
  void embeddedResize(int width, int height, double scaleFactor) {
    takeEnvelope(_ffi.embeddedResize(width, height, scaleFactor));
    final pixels = width * height;
    if (pixels > _framePixels) {
      if (_frame != nullptr) malloc.free(_frame);
      // Zeroed, so the first partial repaint has a defined background.
      _frame = calloc<Uint8>(pixels * 4);
      _framePixels = pixels;
    }
  }

  @override
  Uint8List? embeddedRender(int width, int height) {
    if (_frame == nullptr || width == 0 || height == 0) return null;
    if (!_ffi.embeddedRender(_frame, width, height)) return null;
    return _frame.asTypedList(width * height * 4);
  }

  @override
  int embeddedTick() => _ffi.embeddedTick();

  @override
  bool embeddedHasActiveAnimations() => _ffi.embeddedHasActiveAnimations();

  @override
  void embeddedPointerEvent(
    int kind,
    double x,
    double y,
    int button,
    double deltaX,
    double deltaY,
  ) =>
      takeEnvelope(
        _ffi.embeddedPointerEvent(kind, x, y, button, deltaX, deltaY),
      );

  @override
  void embeddedKeyEvent(int kind, String text) => takeEnvelope(
        withNativeString(text, (t) => _ffi.embeddedKeyEvent(kind, t)),
      );

  @override
  void embeddedFocusEvent(bool focused) =>
      takeEnvelope(_ffi.embeddedFocusEvent(focused));

  @override
  void embeddedDispose() {
    if (_frame != nullptr) malloc.free(_frame);
    _frame = nullptr;
    _framePixels = 0;
  }
}
