// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// `dart:ffi` bindings for the `slint_dart` shared library.
///
/// The entry points are generated into `ffi.g.dart` by ffigen, from the C
/// header cbindgen writes out of `api/flutter/rust`. This file adds the two
/// things a generator cannot: how the library is found at runtime, and the
/// conversions for the JSON envelope both sides speak.
///
/// Everything here is private to the package; [package:slint/slint.dart] is the
/// API users see. The C side is documented in `api/flutter/rust/lib.rs`.
library;

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:ffi/ffi.dart';
import 'package:path/path.dart' as p;

import 'diagnostics.dart';
import 'ffi.g.dart';

export 'ffi.g.dart';

/// The resolved `slint_dart` library.
///
/// [SlintBindings] supplies one method per entry point; this subclass adds
/// only how the library is located.
class SlintFfi extends SlintBindings {
  SlintFfi(super.dynamicLibrary);

  static SlintFfi? _instance;

  /// The process-wide library, opened on first use.
  static SlintFfi get instance => _instance ??= SlintFfi(_openLibrary());

  /// The library inside an application bundle's `Frameworks` directory. The
  /// build hook bundles the `cdylib` as a framework, which is the only copy
  /// an application carries.
  static const frameworkBinary = 'slint_dart.framework/slint_dart';

  /// The file name the Rust `cdylib` gets on this platform. This is the name
  /// cargo writes into `target/`, not the name inside a bundle, so there is
  /// no iOS case: an iOS application has no `target/` directory to look in
  /// and only ever loads [frameworkBinary].
  static String get libraryFileName {
    if (Platform.isWindows) return 'slint_dart.dll';
    if (Platform.isMacOS) return 'libslint_dart.dylib';
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
      // Normalize, since dlopen does not resolve `..` in the path.
      return DynamicLibrary.open(p.normalize(p.absolute(explicit)));
    }
    // An iOS bundle is flat, so its embedded framework sits next to the
    // executable rather than one level up as on macOS. This is where the
    // slice of SlintDart.xcframework ends up, and there is no Cargo
    // `target/` directory on a device to fall back on.
    if (Platform.isIOS) {
      return DynamicLibrary.open(
          '@executable_path/Frameworks/$frameworkBinary');
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
      if (Platform.isMacOS) '@executable_path/../Frameworks/$frameworkBinary',
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
  /// directory, so it loads the [frameworkBinary] the build hook put in the
  /// bundle's `Contents/Frameworks` directory. This path is checked before
  /// the Cargo output directories so bundled builds take precedence over a
  /// stale `target/release` copy.
  static String? _findInBundle() {
    if (Platform.isMacOS) {
      final executable = File(Platform.resolvedExecutable);
      final candidate = File(
        '${executable.parent.path}/../Frameworks/$frameworkBinary',
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
      final packages = (jsonDecode(configFile.readAsStringSync())
          as Map<String, dynamic>)['packages'] as List<Object?>;
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
}

/// Decode a `{"ok": …}` / `{"err": …}` envelope and free it.
///
/// Returns the payload, or throws [SlintException] carrying the message.
Object? takeEnvelope(Pointer<Char> pointer) {
  final envelope = jsonDecode(takeString(pointer)) as Map<String, dynamic>;
  final error = envelope['err'];
  if (error != null) throw SlintException(error as String);
  return envelope['ok'];
}

/// Copy a string the Rust side allocated into Dart, and free the original.
///
/// The generated bindings speak `Pointer<Char>`, which is the same address as
/// `package:ffi`'s `Pointer<Utf8>`; the cast is what lets the conversion
/// helpers in this file stay the only place that knows about the encoding.
String takeString(Pointer<Char> pointer) {
  if (pointer == nullptr) return '';
  final value = pointer.cast<Utf8>().toDartString();
  SlintFfi.instance.freeString(pointer);
  return value;
}

/// Run [body] with `value` as a native string, freeing it afterwards. A null
/// [value] is passed through as a null pointer, which the C side reads as
/// "not set".
T withNativeString<T>(String? value, T Function(Pointer<Char>) body) {
  if (value == null) return body(nullptr);
  final native = value.toNativeUtf8();
  try {
    return body(native.cast<Char>());
  } finally {
    malloc.free(native);
  }
}
