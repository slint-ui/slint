// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore rustup

import 'dart:convert';
import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/hooks.dart';

/// The name of the code asset: the cdylib built by the `slint-dart` crate.
const assetName = 'libslint_dart';

/// The Rust target triple for each supported operating system and
/// architecture pair. `null` means the combination isn't supported yet.
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

/// The file name extension of the dynamic library for [os].
String libraryExtension(OS os) {
  return switch (os) {
    OS.macOS => 'dylib',
    OS.linux => 'so',
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
  final home = Platform.environment['HOME'] ?? Platform.environment['USERPROFILE'];
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
    if (code.linkModePreference == LinkModePreference.static) {
      throw UnsupportedError(
        'Slint only ships a dynamic library; static linking is not supported.',
      );
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

    final profile = cargoProfile(input.userDefines['cargo_profile']);
    final crateRoot = input.packageRoot.resolve('../');
    final library = await cargoBuild(crateRoot, profile);

    final assetFile =
        input.outputDirectory.resolve('$assetName.${libraryExtension(code.targetOS)}');
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
  });
}
