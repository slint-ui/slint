// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// cSpell: ignore rustup

import 'dart:convert';
import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/hooks.dart';

/// The name of the code asset: the cdylib built by the `slint-dart` crate.
const assetName = 'libslint_dart';

/// The Rust target triple for each supported operating system and
/// architecture pair. `null` means the combination isn't supported yet.
///
/// Android never appears here: its libraries are cross-compiled with
/// `cargo-ndk`, which takes an ABI name instead of a target triple, so it is
/// handled by [cargoNdkAbi] rather than through this table.
String? rustTarget(OS os, Architecture architecture) {
  return switch ((os.name, architecture.name)) {
    ('macos', 'arm64') => 'aarch64-apple-darwin',
    ('macos', 'x64') => 'x86_64-apple-darwin',
    ('linux', 'arm64') => 'aarch64-unknown-linux-gnu',
    ('linux', 'x64') => 'x86_64-unknown-linux-gnu',
    ('windows', 'arm64') => 'aarch64-pc-windows-msvc',
    ('windows', 'x64') => 'x86_64-pc-windows-msvc',
    _ => null,
  };
}

/// The `cargo-ndk` ABI name for an Android [Architecture]. `null` means the
/// combination isn't supported: it must be one of the ABIs the Flutter tool
/// places in `jniLibs`, which this version of Flutter limits to
/// `armeabi-v7a`, `arm64-v8a` and `x86_64` (there is no 32-bit x86 slice).
String? cargoNdkAbi(Architecture architecture) {
  return switch (architecture.name) {
    'arm' => 'armeabi-v7a',
    'arm64' => 'arm64-v8a',
    'x64' => 'x86_64',
    _ => null,
  };
}

/// The file name extension of the dynamic library for [os].
String libraryExtension(OS os) {
  return switch (os) {
    OS.macOS => 'dylib',
    OS.linux => 'so',
    OS.android => 'so',
    OS.windows => 'dll',
    _ => throw UnsupportedError(
        'Slint does not bundle a dynamic library for ${os.name} yet.',
      ),
  };
}

/// Resolve a tool by name: on `PATH` first, then in the standard rustup
/// location under the user's home directory. The hook environment passes
/// `PATH` and `HOME` through, but does not add `~/.cargo/bin`.
Future<String?> findTool(String name) async {
  final onPath = await Process.run(
    Platform.isWindows ? 'where' : 'which',
    [name],
  );
  if (onPath.exitCode == 0) return name;
  final home =
      Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
  if (home != null) {
    final candidate = File('$home${Platform.pathSeparator}.cargo'
        '${Platform.pathSeparator}bin${Platform.pathSeparator}$name');
    if (candidate.existsSync()) return candidate.path;
  }
  return null;
}

/// The host's Rust target triple, from `rustc -vV`, or null when the Rust
/// toolchain isn't available.
Future<String?> hostRustTarget() async {
  final rustc = await findTool('rustc');
  if (rustc == null) return null;
  final result = await Process.run(rustc, ['-vV']);
  if (result.exitCode != 0) return null;
  for (final line in (result.stdout as String).split('\n')) {
    if (line.startsWith('host: ')) {
      return line.substring('host: '.length).trim();
    }
  }
  return null;
}

/// The cargo profile to build: `release` by default, or the value of the
/// `cargo_profile` user-define. Debug builds are faster to produce, release
/// builds are what the README's `cargo build --release -p slint-dart`
/// documents.
String cargoProfile(Object? value) {
  if (value == null) return 'release';
  if (value == 'debug' || value == 'release') return value as String;
  throw const FormatException(
    "hooks.user_defines.slint.cargo_profile must be 'debug' or 'release'",
  );
}

/// Build `slint-dart` with cargo and answer with the URI of the produced
/// dynamic library.
Future<Uri> cargoBuild(Uri crateRoot, String profile) async {
  final cargo = await findTool('cargo');
  if (cargo == null) {
    throw StateError(
      'Building the Slint native library needs the Rust toolchain '
      '(cargo and rustc). Install it with rustup (https://rustup.rs), or '
      'build libslint_dart yourself and set SLINT_DART_LIBRARY to it.',
    );
  }
  final arguments = [
    'build',
    if (profile == 'release') '--release',
    '-p',
    'slint-dart',
  ];
  final result = await Process.run(
    cargo,
    arguments,
    workingDirectory: crateRoot.toFilePath(),
  );
  if (result.exitCode != 0) {
    throw StateError(
      'cargo ${arguments.join(' ')} failed with exit code '
      '${result.exitCode}:\n${result.stderr}',
    );
  }
  final extension = switch (Platform.operatingSystem) {
    'macos' => 'dylib',
    'linux' => 'so',
    'windows' => 'dll',
    _ => throw UnsupportedError(
        'Slint does not bundle a dynamic library on '
        '${Platform.operatingSystem} yet.',
      ),
  };
  // The slint-dart crate is a workspace member, so the artifact lands in the
  // workspace's shared target directory, not next to the crate.
  final metadata = await Process.run(
    cargo,
    ['metadata', '--format-version', '1', '--no-deps'],
    workingDirectory: crateRoot.toFilePath(),
  );
  if (metadata.exitCode != 0) {
    throw StateError(
      'cargo metadata failed with exit code ${metadata.exitCode}:\n'
      '${metadata.stderr}',
    );
  }
  final targetDirectory = (jsonDecode(metadata.stdout as String)
      as Map<String, dynamic>)['target_directory'] as String;
  return Uri.file('$targetDirectory/$profile/$assetName.$extension');
}

/// Build `slint-dart` for Android with `cargo-ndk` and answer with the URI of
/// the produced shared library.
///
/// Android is always a cross-compile, so the plain `cargo build` path cannot be
/// reused: `cargo-ndk` supplies the linker and the Rust target through the
/// Android NDK. It writes each ABI's library into `<output>/<abi>/`, and the
/// caller copies out the single `.so` for the ABI this hook invocation was
/// asked for.
Future<Uri> androidCargoBuild(
  Uri crateRoot,
  String abi,
  String profile,
  int targetNdkApi,
) async {
  final cargo = await findTool('cargo');
  if (cargo == null) {
    throw StateError(
      'Building the Slint native library for Android needs the Rust '
      'toolchain (cargo and rustc) plus the cargo-ndk subcommand (install it '
      'with `cargo install cargo-ndk`) and the Android NDK. Alternatively '
      'build libslint_dart yourself and set SLINT_DART_LIBRARY to it.',
    );
  }
  // A scratch directory that `cargo-ndk` lays out as `jniLibs`; only the ABI
  // we were asked for is produced, and the single `.so` is copied out by the
  // caller. The directory is a system temp dir, so leaving it behind is fine.
  final staging = Directory.systemTemp.createTempSync('slint-ndk-');
  final arguments = [
    'ndk',
    '-t',
    abi,
    '-o',
    staging.path,
    // The minimum Android API level to target (`-P`, not `-p`, which cargo
    // reads as a package selector).
    '-P',
    '$targetNdkApi',
    'build',
    if (profile == 'release') '--release',
    // Android always draws through the embedded SlintSurface, so only the
    // software renderer is needed; winit/Skia/FemtoVG cannot cross-compile
    // here and would be dead weight. This mirrors the xcframework script.
    '--no-default-features',
    '--features',
    'renderer-software',
    '-p',
    'slint-dart',
  ];
  final result = await Process.run(
    cargo,
    arguments,
    workingDirectory: crateRoot.toFilePath(),
  );
  if (result.exitCode != 0) {
    throw StateError(
      'cargo ${arguments.join(' ')} failed with exit code '
      '${result.exitCode}:\n${result.stderr}',
    );
  }
  return Uri.file('${staging.path}/$abi/$assetName.so');
}

/// The workspace root of the `slint-dart` crate: the nearest ancestor that
/// has a `Cargo.lock` (the repository root), or null when there is none.
Uri? workspaceRoot(Uri crateRoot) {
  var directory = crateRoot;
  while (true) {
    final lockFile = File.fromUri(directory.resolve('Cargo.lock'));
    if (lockFile.existsSync()) return directory;
    final parent = directory.resolve('../');
    if (parent == directory) return null;
    directory = parent;
  }
}

void main(List<String> arguments) async {
  await build(arguments, (input, output) async {
    if (!input.config.buildCodeAssets) return;

    final code = input.config.code;

    // iOS arrives as SlintDart.xcframework instead of being built here: the
    // hook cannot cross-compile from the host, and an embedded framework is
    // what an iOS application can load. Build it with
    // `scripts/build_slint_dart_xcframework.bash` and embed it in the Runner
    // target; `package:slint` opens it from the app bundle at runtime.
    if (code.targetOS == OS.iOS) return;

    if (code.linkModePreference == LinkModePreference.static) {
      throw UnsupportedError(
        'Slint only ships a dynamic library; static linking is not supported.',
      );
    }

    final profile = cargoProfile(input.userDefines['cargo_profile']);
    final crateRoot = input.packageRoot.resolve('../');

    // Android is always a cross-compile, so it cannot go through the host
    // `cargo build` path below; build the single ABI this invocation was asked
    // for with `cargo-ndk` against the Android NDK.
    if (code.targetOS == OS.android) {
      final abi = cargoNdkAbi(code.targetArchitecture);
      if (abi == null) {
        throw UnsupportedError(
          'Slint does not support building for Android '
          '${code.targetArchitecture.name} yet.',
        );
      }
      final targetNdkApi = code.android.targetNdkApi;
      final library = await androidCargoBuild(
        crateRoot,
        abi,
        profile,
        targetNdkApi,
      );
      await _emitAsset(input, output, code, crateRoot, library);
      return;
    }

    final target = rustTarget(code.targetOS, code.targetArchitecture);
    if (target == null) {
      throw UnsupportedError(
        'Slint does not support building for '
        '${code.targetOS.name}/${code.targetArchitecture.name} yet.',
      );
    }
    final host = await hostRustTarget();
    if (host == null) {
      throw StateError(
        'Building the Slint native library needs the Rust toolchain '
        '(cargo and rustc). Install it with rustup (https://rustup.rs), or '
        'build libslint_dart yourself and set SLINT_DART_LIBRARY to it.',
      );
    }
    if (target != host) {
      throw UnsupportedError(
        'Cross-compiling slint-dart from $host to $target is not supported '
        'yet. Build the application for the host platform, or build '
        'libslint_dart yourself and set SLINT_DART_LIBRARY to it.',
      );
    }

    final library = await cargoBuild(crateRoot, profile);

    await _emitAsset(input, output, code, crateRoot, library);
  });
}

/// Copy [library] into the hook's output directory and register it as a
/// bundled code asset, alongside the workspace files that influence the build.
Future<void> _emitAsset(
  BuildInput input,
  BuildOutputBuilder output,
  CodeConfig code,
  Uri crateRoot,
  Uri library,
) async {
  final assetFile = input.outputDirectory
      .resolve('$assetName.${libraryExtension(code.targetOS)}');
  await File.fromUri(library).copy(assetFile.toFilePath());

  // Declare everything that influences the build, so the hook cache
  // invalidates when the Rust sources or the workspace change.
  output.dependencies
    ..add(crateRoot.resolve('Cargo.toml'))
    ..add(crateRoot.resolve('rust'));
  final root = workspaceRoot(crateRoot);
  if (root != null) {
    output.dependencies
      ..add(root.resolve('Cargo.lock'))
      ..add(root.resolve('Cargo.toml'));
  }

  output.assets.code.add(
    CodeAsset(
      package: input.packageName,
      name: assetName,
      linkMode: DynamicLoadingBundled(),
      file: assetFile,
    ),
  );
}
