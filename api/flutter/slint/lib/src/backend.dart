// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/// What the Dart runtime needs from the `slint_dart` library, with the
/// platform's marshalling hidden behind it.
///
/// There are two implementations. On desktop and mobile the library is a
/// `dylib` reached through `dart:ffi`; on the web it is a WebAssembly module
/// reached through `dart:js_interop`. Everything above this line — property
/// access, callback dispatch, JSON conversion, error handling — is written
/// once in [package:slint/slint.dart] against this interface.
///
/// Handles are plain integers: a pointer address natively, a wasm address on
/// the web. They only mean something to the implementation that produced them.
library;

import 'dart:convert';
import 'dart:typed_data';

import 'backend_native.dart' if (dart.library.js_interop) 'backend_web.dart'
    as impl;
import 'diagnostics.dart';

/// The backend for this platform, created on first use.
SlintBackend get backend => _backend ??= impl.createBackend();
SlintBackend? _backend;

/// Runs a Slint callback and returns its result as JSON, or null for "void".
typedef CallbackDispatch = String? Function(int id, String argsJson);

/// Runs a Slint timer callback.
typedef TimerDispatch = void Function(int id);

/// Decode a `{"ok": …}` / `{"err": …}` envelope, the reply shape every
/// fallible entry point uses.
///
/// Returns the payload, or throws [SlintException] carrying the message.
Object? decodeEnvelope(String json) {
  if (json.isEmpty) return null;
  final envelope = jsonDecode(json) as Map<String, dynamic>;
  final error = envelope['err'];
  if (error != null) throw SlintException(error as String);
  return envelope['ok'];
}

abstract interface class SlintBackend {
  /// Make the library usable. On the web this fetches and instantiates the
  /// WebAssembly module served at [scriptUrl]; everywhere else the library is
  /// already loaded and this returns immediately.
  Future<void> initialize({String? scriptUrl});

  /// Install the functions Slint calls back into Dart with. Called once,
  /// before any callback or timer is registered.
  void installDispatchers(CallbackDispatch onCallback, TimerDispatch onTimer);

  /// Report a failure that escaped a Dart handler, where throwing is not an
  /// option because the call came from native code.
  void reportError(String message);

  // Compiler ---------------------------------------------------------------

  int compilerNew();
  void compilerFree(int compiler);
  void compilerSetStyle(int compiler, String style);
  void compilerAddIncludePath(int compiler, String path);

  /// Compile a `.slint` file. The web has no filesystem, so its implementation
  /// throws [SlintException] and applications there use [buildFromSource].
  int buildFromPath(int compiler, String path);

  int buildFromSource(int compiler, String source, String path);

  // Compilation result -----------------------------------------------------

  void resultFree(int result);
  bool resultHasErrors(int result);
  Object? resultDiagnostics(int result);
  Object? resultComponentNames(int result);

  /// The named component, or the last exported one when [name] is null.
  /// Returns 0 when there is no such component.
  int resultComponent(int result, String? name);

  // Component definition ---------------------------------------------------

  void definitionFree(int definition);

  /// Instantiate the component, or throw [SlintException] with the reason.
  int definitionCreate(int definition);

  // Component instance -----------------------------------------------------

  void instanceFree(int instance);
  Object? getProperty(int instance, String? global, String name);
  void setProperty(int instance, String? global, String name, String json);
  Object? invoke(int instance, String? global, String name, String argsJson);

  /// Route the callback to the Dart handler registered under [id].
  void setCallback(int instance, String? global, String name, int id);

  void instanceShow(int instance, bool visible);

  // Event loop -------------------------------------------------------------

  /// Show the window and run the event loop. The web has no such loop — the
  /// browser owns it — so its implementation throws [SlintException].
  void instanceRun(int instance);

  void runEventLoop();
  void quitEventLoop();

  // Timers -----------------------------------------------------------------

  int timerStart(bool repeated, int intervalMs, int id);
  void timerFree(int timer);

  // Embedded rendering -----------------------------------------------------

  void embeddedInit();
  void embeddedResize(int width, int height, double scaleFactor);

  /// Draw the next frame, or return null when nothing needed repainting. The
  /// result is only valid until the next call.
  Uint8List? embeddedRender(int width, int height);

  int embeddedTick();
  bool embeddedHasActiveAnimations();
  void embeddedPointerEvent(
    int kind,
    double x,
    double y,
    int button,
    double deltaX,
    double deltaY,
  );
  void embeddedKeyEvent(int kind, String text);
  void embeddedFocusEvent(bool focused);

  /// Release what the renderer holds. The library itself stays loaded.
  void embeddedDispose();
}
