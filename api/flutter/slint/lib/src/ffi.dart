// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// `dart:ffi` bindings for the `slint_dart` shared library.
///
/// Everything here is private to the package; [package:slint/slint.dart] is the
/// API users see. The C side is documented in `api/flutter/rust/lib.rs`.
library;

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';

import 'diagnostics.dart';

/// Opaque handle types. They exist only to keep the pointers apart in the
/// Dart type system; their contents live on the Rust side.
final class SlintCompiler extends Opaque {}

final class SlintCompilationResult extends Opaque {}

final class SlintComponentDefinition extends Opaque {}

final class SlintComponentInstance extends Opaque {}

final class SlintTimerHandle extends Opaque {}

typedef CallbackNative = Pointer<Utf8> Function(Pointer<Void>, Pointer<Utf8>);
typedef FreeNative = Void Function(Pointer<Utf8>);
typedef TimerNative = Void Function(Pointer<Void>);

/// The resolved `slint_dart` library, with one getter per entry point.
class SlintFfi {
  SlintFfi(this._lib);

  final DynamicLibrary _lib;

  static SlintFfi? _instance;

  /// The process-wide library, opened on first use.
  static SlintFfi get instance => _instance ??= SlintFfi(_openLibrary());

  /// The file name the Rust `cdylib` gets on this platform.
  static String get libraryFileName {
    if (Platform.isWindows) return 'slint_dart.dll';
    if (Platform.isMacOS || Platform.isIOS) return 'libslint_dart.dylib';
    return 'libslint_dart.so';
  }

  /// Find `libslint_dart`.
  ///
  /// `SLINT_DART_LIBRARY` wins when set. Otherwise this looks through the
  /// Cargo output directories above the current directory, the running
  /// executable, the running script, and the root of the linked `slint`
  /// package. The package root covers `flutter run`: a desktop app's working
  /// directory, executable, and script are usually outside the repository,
  /// but its package config still points at the `slint` checkout, whose
  /// ancestors hold the Cargo `target/` directory. As a last resort the
  /// platform loader gets a chance, which is the path a packaged application
  /// takes.
  static DynamicLibrary _openLibrary() {
    final explicit = Platform.environment['SLINT_DART_LIBRARY'];
    if (explicit != null && explicit.isNotEmpty) {
      return DynamicLibrary.open(explicit);
    }
    final bundled = _findInBundle();
    if (bundled != null) {
      return DynamicLibrary.open(bundled);
    }
    final found = _findInCargoTarget();
    if (found != null) {
      return DynamicLibrary.open(found);
    }
    // As a last resort the platform loader gets a chance, which is the path
    // a packaged application takes. Flutter bundles the library built by the
    // build hook: on macOS it becomes a framework inside the app bundle.
    final attempts = <String>[
      libraryFileName,
      if (Platform.isMacOS)
        '@executable_path/../Frameworks/slint_dart.framework/slint_dart',
    ];
    Object? lastError;
    for (final name in attempts) {
      try {
        return DynamicLibrary.open(name);
      } on ArgumentError catch (e) {
        lastError = e;
      }
    }
    throw StateError(
      'Cannot find $libraryFileName. Build it with\n'
      '    cargo build --release -p slint-dart\n'
      'or point SLINT_DART_LIBRARY at an existing copy.\n'
      'Underlying error: $lastError',
    );
  }

  /// Look for the library inside the current application bundle.
  ///
  /// On macOS a sandboxed Flutter app cannot access the Cargo `target/`
  /// directory, so copy `libslint_dart.dylib` into the bundle's
  /// `Contents/Frameworks` directory. This path is checked before the Cargo
  /// output directories so bundled builds take precedence.
  static String? _findInBundle() {
    if (Platform.isMacOS || Platform.isIOS) {
      final executable = File(Platform.resolvedExecutable);
      final candidate = File(
        '${executable.parent.path}/../Frameworks/$libraryFileName',
      );
      if (candidate.existsSync()) {
        return candidate.resolveSymbolicLinksSync();
      }
    }
    return null;
  }

  static String? _findInCargoTarget() {
    final roots = <String>{
      Directory.current.path,
      File(Platform.resolvedExecutable).parent.path,
      if (Platform.script.scheme == 'file')
        File.fromUri(Platform.script).parent.path,
      ..._linkedPackageRoots(),
    };
    for (final root in roots) {
      for (var dir = Directory(root);; dir = dir.parent) {
        for (final profile in const ['release', 'debug']) {
          final candidate = File(
            '${dir.path}${Platform.pathSeparator}target'
            '${Platform.pathSeparator}$profile'
            '${Platform.pathSeparator}$libraryFileName',
          );
          if (candidate.existsSync()) return candidate.path;
        }
        if (dir.parent.path == dir.path) break;
      }
    }
    return null;
  }

  /// The root of every package this isolate links against, read from its
  /// package config. The `slint` package's own root is the useful entry:
  /// with a path dependency it is a checkout whose ancestors contain the
  /// Cargo `target/` directory, which the in-tree examples and path-based
  /// applications rely on. Malformed or missing configs yield nothing.
  static Iterable<String> _linkedPackageRoots() {
    final config = Platform.packageConfig;
    if (config == null) return const [];
    try {
      final configFile = File.fromUri(Uri.parse(config));
      final packages =
          (jsonDecode(configFile.readAsStringSync()) as Map<String, dynamic>)
              ['packages'] as List<Object?>;
      return [
        for (final entry in packages.whereType<Map<String, dynamic>>())
          if (entry['name'] == 'slint') _packageRoot(entry, configFile),
      ].whereType<String>();
    } on Object {
      return const [];
    }
  }

  static String? _packageRoot(Map<String, dynamic> entry, File configFile) {
    final rootUri = entry['rootUri'];
    if (rootUri is! String) return null;
    final uri = Uri.tryParse(rootUri);
    if (uri == null) return null;
    final resolved =
        uri.isAbsolute ? uri : configFile.parent.uri.resolveUri(uri);
    if (resolved.scheme != 'file') return null;
    return File.fromUri(resolved).path;
  }

  late final void Function(Pointer<Utf8>) freeString = _lib.lookupFunction<
      Void Function(Pointer<Utf8>), void Function(Pointer<Utf8>)>(
    'slint_dart_free_string',
  );

  late final Pointer<SlintCompiler> Function() compilerNew =
      _lib.lookupFunction<Pointer<SlintCompiler> Function(),
          Pointer<SlintCompiler> Function()>('slint_dart_compiler_new');

  late final void Function(Pointer<SlintCompiler>) compilerFree =
      _lib.lookupFunction<Void Function(Pointer<SlintCompiler>),
          void Function(Pointer<SlintCompiler>)>('slint_dart_compiler_free');

  late final void Function(Pointer<SlintCompiler>, Pointer<Utf8>)
      compilerSetStyle = _lib.lookupFunction<
          Void Function(Pointer<SlintCompiler>, Pointer<Utf8>),
          void Function(Pointer<SlintCompiler>, Pointer<Utf8>)>(
    'slint_dart_compiler_set_style',
  );

  late final void Function(Pointer<SlintCompiler>, Pointer<Utf8>)
      compilerAddIncludePath = _lib.lookupFunction<
          Void Function(Pointer<SlintCompiler>, Pointer<Utf8>),
          void Function(Pointer<SlintCompiler>, Pointer<Utf8>)>(
    'slint_dart_compiler_add_include_path',
  );

  late final Pointer<SlintCompilationResult> Function(
      Pointer<SlintCompiler>, Pointer<Utf8>) buildFromPath = _lib.lookupFunction<
          Pointer<SlintCompilationResult> Function(
              Pointer<SlintCompiler>, Pointer<Utf8>),
          Pointer<SlintCompilationResult> Function(
              Pointer<SlintCompiler>, Pointer<Utf8>)>(
    'slint_dart_compiler_build_from_path',
  );

  late final Pointer<SlintCompilationResult> Function(
          Pointer<SlintCompiler>, Pointer<Utf8>, Pointer<Utf8>)
      buildFromSource = _lib.lookupFunction<
          Pointer<SlintCompilationResult> Function(
              Pointer<SlintCompiler>, Pointer<Utf8>, Pointer<Utf8>),
          Pointer<SlintCompilationResult> Function(
              Pointer<SlintCompiler>, Pointer<Utf8>, Pointer<Utf8>)>(
    'slint_dart_compiler_build_from_source',
  );

  late final Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>)
      generate = _lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>),
          Pointer<Utf8> Function(Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>)>(
    'slint_dart_generate',
  );

  late final void Function(Pointer<SlintCompilationResult>) resultFree =
      _lib.lookupFunction<Void Function(Pointer<SlintCompilationResult>),
          void Function(Pointer<SlintCompilationResult>)>(
    'slint_dart_result_free',
  );

  late final bool Function(Pointer<SlintCompilationResult>) resultHasErrors =
      _lib.lookupFunction<Bool Function(Pointer<SlintCompilationResult>),
          bool Function(Pointer<SlintCompilationResult>)>(
    'slint_dart_result_has_errors',
  );

  late final Pointer<Utf8> Function(Pointer<SlintCompilationResult>)
      resultDiagnostics = _lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<SlintCompilationResult>),
          Pointer<Utf8> Function(Pointer<SlintCompilationResult>)>(
    'slint_dart_result_diagnostics',
  );

  late final Pointer<Utf8> Function(Pointer<SlintCompilationResult>)
      resultComponentNames = _lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<SlintCompilationResult>),
          Pointer<Utf8> Function(Pointer<SlintCompilationResult>)>(
    'slint_dart_result_component_names',
  );

  late final Pointer<SlintComponentDefinition> Function(
          Pointer<SlintCompilationResult>, Pointer<Utf8>) resultComponent =
      _lib.lookupFunction<
          Pointer<SlintComponentDefinition> Function(
              Pointer<SlintCompilationResult>, Pointer<Utf8>),
          Pointer<SlintComponentDefinition> Function(
              Pointer<SlintCompilationResult>, Pointer<Utf8>)>(
    'slint_dart_result_component',
  );

  late final void Function(Pointer<SlintComponentDefinition>) definitionFree =
      _lib.lookupFunction<Void Function(Pointer<SlintComponentDefinition>),
          void Function(Pointer<SlintComponentDefinition>)>(
    'slint_dart_definition_free',
  );

  late final Pointer<Utf8> Function(Pointer<SlintComponentDefinition>)
      definitionName = _lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<SlintComponentDefinition>),
          Pointer<Utf8> Function(Pointer<SlintComponentDefinition>)>(
    'slint_dart_definition_name',
  );

  late final Pointer<Utf8> Function(Pointer<SlintComponentDefinition>)
      definitionApi = _lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<SlintComponentDefinition>),
          Pointer<Utf8> Function(Pointer<SlintComponentDefinition>)>(
    'slint_dart_definition_api',
  );

  late final Pointer<SlintComponentInstance> Function(
          Pointer<SlintComponentDefinition>, Pointer<Pointer<Utf8>>)
      definitionCreate = _lib.lookupFunction<
          Pointer<SlintComponentInstance> Function(
              Pointer<SlintComponentDefinition>, Pointer<Pointer<Utf8>>),
          Pointer<SlintComponentInstance> Function(
              Pointer<SlintComponentDefinition>, Pointer<Pointer<Utf8>>)>(
    'slint_dart_definition_create',
  );

  late final void Function(Pointer<SlintComponentInstance>) instanceFree =
      _lib.lookupFunction<Void Function(Pointer<SlintComponentInstance>),
          void Function(Pointer<SlintComponentInstance>)>(
    'slint_dart_instance_free',
  );

  late final Pointer<Utf8> Function(
          Pointer<SlintComponentInstance>, Pointer<Utf8>, Pointer<Utf8>)
      getProperty = _lib.lookupFunction<
          Pointer<Utf8> Function(
              Pointer<SlintComponentInstance>, Pointer<Utf8>, Pointer<Utf8>),
          Pointer<Utf8> Function(Pointer<SlintComponentInstance>, Pointer<Utf8>,
              Pointer<Utf8>)>('slint_dart_instance_get_property');

  late final Pointer<Utf8> Function(Pointer<SlintComponentInstance>,
          Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>) setProperty =
      _lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<SlintComponentInstance>, Pointer<Utf8>,
              Pointer<Utf8>, Pointer<Utf8>),
          Pointer<Utf8> Function(Pointer<SlintComponentInstance>, Pointer<Utf8>,
              Pointer<Utf8>, Pointer<Utf8>)>('slint_dart_instance_set_property');

  late final Pointer<Utf8> Function(Pointer<SlintComponentInstance>,
          Pointer<Utf8>, Pointer<Utf8>, Pointer<Utf8>) invoke =
      _lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<SlintComponentInstance>, Pointer<Utf8>,
              Pointer<Utf8>, Pointer<Utf8>),
          Pointer<Utf8> Function(Pointer<SlintComponentInstance>, Pointer<Utf8>,
              Pointer<Utf8>, Pointer<Utf8>)>('slint_dart_instance_invoke');

  late final Pointer<Utf8> Function(
      Pointer<SlintComponentInstance>,
      Pointer<Utf8>,
      Pointer<Utf8>,
      Pointer<NativeFunction<CallbackNative>>,
      Pointer<NativeFunction<FreeNative>>,
      Pointer<Void>) setCallback = _lib.lookupFunction<
          Pointer<Utf8> Function(
              Pointer<SlintComponentInstance>,
              Pointer<Utf8>,
              Pointer<Utf8>,
              Pointer<NativeFunction<CallbackNative>>,
              Pointer<NativeFunction<FreeNative>>,
              Pointer<Void>),
          Pointer<Utf8> Function(
              Pointer<SlintComponentInstance>,
              Pointer<Utf8>,
              Pointer<Utf8>,
              Pointer<NativeFunction<CallbackNative>>,
              Pointer<NativeFunction<FreeNative>>,
              Pointer<Void>)>('slint_dart_instance_set_callback');

  late final Pointer<Utf8> Function(Pointer<SlintComponentInstance>, bool)
      instanceShow = _lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<SlintComponentInstance>, Bool),
          Pointer<Utf8> Function(Pointer<SlintComponentInstance>, bool)>(
    'slint_dart_instance_show',
  );

  late final Pointer<Utf8> Function(Pointer<SlintComponentInstance>)
      instanceRun = _lib.lookupFunction<
          Pointer<Utf8> Function(Pointer<SlintComponentInstance>),
          Pointer<Utf8> Function(Pointer<SlintComponentInstance>)>(
    'slint_dart_instance_run',
  );

  late final Pointer<Utf8> Function() runEventLoop =
      _lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>(
    'slint_dart_run_event_loop',
  );

  late final void Function() quitEventLoop =
      _lib.lookupFunction<Void Function(), void Function()>(
    'slint_dart_quit_event_loop',
  );

  late final Pointer<SlintTimerHandle> Function(bool, int,
          Pointer<NativeFunction<TimerNative>>, Pointer<Void>) timerStart =
      _lib.lookupFunction<
          Pointer<SlintTimerHandle> Function(Bool, Uint64,
              Pointer<NativeFunction<TimerNative>>, Pointer<Void>),
          Pointer<SlintTimerHandle> Function(bool, int,
              Pointer<NativeFunction<TimerNative>>, Pointer<Void>)>(
    'slint_dart_timer_start',
  );

  late final void Function(Pointer<SlintTimerHandle>) timerFree =
      _lib.lookupFunction<Void Function(Pointer<SlintTimerHandle>),
          void Function(Pointer<SlintTimerHandle>)>('slint_dart_timer_free');

  // Embedded mode — see rust/embedded.rs.

  late final Pointer<Utf8> Function() embeddedInit =
      _lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>(
    'slint_dart_embedded_init',
  );

  late final Pointer<Utf8> Function(int, int, double) embeddedResize =
      _lib.lookupFunction<Pointer<Utf8> Function(Uint32, Uint32, Float),
          Pointer<Utf8> Function(int, int, double)>(
    'slint_dart_embedded_resize',
  );

  late final bool Function(Pointer<Uint8>, int, int) embeddedRender =
      _lib.lookupFunction<Bool Function(Pointer<Uint8>, Uint32, Uint32),
          bool Function(Pointer<Uint8>, int, int)>(
    'slint_dart_embedded_render',
  );

  late final int Function() embeddedTick =
      _lib.lookupFunction<Int64 Function(), int Function()>(
    'slint_dart_embedded_tick',
  );

  late final bool Function() embeddedHasActiveAnimations =
      _lib.lookupFunction<Bool Function(), bool Function()>(
    'slint_dart_embedded_has_active_animations',
  );

  late final Pointer<Utf8> Function(int, double, double, int, double, double)
      embeddedPointerEvent = _lib.lookupFunction<
          Pointer<Utf8> Function(Uint32, Float, Float, Uint32, Float, Float),
          Pointer<Utf8> Function(int, double, double, int, double, double)>(
    'slint_dart_embedded_pointer_event',
  );

  late final Pointer<Utf8> Function(int, Pointer<Utf8>) embeddedKeyEvent =
      _lib.lookupFunction<Pointer<Utf8> Function(Uint32, Pointer<Utf8>),
          Pointer<Utf8> Function(int, Pointer<Utf8>)>(
    'slint_dart_embedded_key_event',
  );

  late final Pointer<Utf8> Function(bool) embeddedFocusEvent =
      _lib.lookupFunction<Pointer<Utf8> Function(Bool),
          Pointer<Utf8> Function(bool)>('slint_dart_embedded_focus_event');

  late final Pointer<Utf8> Function() embeddedSize =
      _lib.lookupFunction<Pointer<Utf8> Function(), Pointer<Utf8> Function()>(
    'slint_dart_embedded_size',
  );
}

/// Decode a `{"ok": …}` / `{"err": …}` envelope and free it.
///
/// Returns the payload, or throws [SlintException] carrying the message.
Object? takeEnvelope(Pointer<Utf8> pointer) {
  final envelope = jsonDecode(takeString(pointer)) as Map<String, dynamic>;
  final error = envelope['err'];
  if (error != null) throw SlintException(error as String);
  return envelope['ok'];
}

/// Copy a string the Rust side allocated into Dart, and free the original.
String takeString(Pointer<Utf8> pointer) {
  if (pointer == nullptr) return '';
  final value = pointer.toDartString();
  SlintFfi.instance.freeString(pointer);
  return value;
}

/// Run [body] with `value` as a native string, freeing it afterwards. A null
/// [value] is passed through as a null pointer, which the C side reads as
/// "not set".
T withNativeString<T>(String? value, T Function(Pointer<Utf8>) body) {
  if (value == null) return body(nullptr);
  final native = value.toNativeUtf8();
  try {
    return body(native);
  } finally {
    malloc.free(native);
  }
}
