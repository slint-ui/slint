// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import 'dart:ffi' show Abi;
import 'dart:io';

import 'package:code_assets/code_assets.dart';
import 'package:hooks/src/test.dart' as hooks_test;
import 'package:test/test.dart';

import '../hook/build.dart' as build_hook;

/// The host OS in code-asset terms, or null when the hook doesn't support it.
OS? hostOs(Abi abi) {
  return switch (abi) {
    Abi.macosArm64 || Abi.macosX64 => OS.macOS,
    Abi.linuxArm64 || Abi.linuxX64 => OS.linux,
    Abi.windowsArm64 || Abi.windowsX64 => OS.windows,
    _ => null,
  };
}

void main() {
  test('the cargo profile defaults to release and honors the user-define', () {
    expect(build_hook.cargoProfile(null), 'release');
    expect(build_hook.cargoProfile('debug'), 'debug');
    expect(build_hook.cargoProfile('release'), 'release');
    expect(
      () => build_hook.cargoProfile('fast'),
      throwsA(isA<FormatException>()),
    );
  });

  test('leaves iOS to the xcframework: no asset, no build', () async {
    await hooks_test.testBuildHook(
      mainMethod: build_hook.main,
      extensions: [
        CodeAssetExtension(
          targetOS: OS.iOS,
          targetArchitecture: Architecture.arm64,
          linkModePreference: LinkModePreference.dynamic,
          iOS: IOSCodeConfig(targetSdk: IOSSdk.iPhoneOS, targetVersion: 13),
        ),
      ],
      check: (input, output) {
        expect(
            output.assets.encodedAssets.where((a) => a.isCodeAsset), isEmpty);
        expect(output.dependencies, isEmpty);
      },
    );
  });

  test('builds libslint_dart and declares it as a bundled code asset',
      () async {
    final abi = Abi.current();
    final os = hostOs(abi);
    final hostTriple = await build_hook.hostRustTarget();
    if (os == null || hostTriple == null) {
      markTestSkipped(
        'The host (${Platform.operatingSystem}/${Platform.version}) is not '
        'supported, or the Rust toolchain is unavailable.',
      );
      return;
    }

    // The hook builds with the default `release` cargo profile, which never
    // touches the `target/debug` library that the other tests load with
    // `--features backend-testing`.
    await hooks_test.testBuildHook(
      mainMethod: build_hook.main,
      extensions: [
        CodeAssetExtension(
          targetOS: os,
          targetArchitecture: Architecture.fromAbi(abi),
          linkModePreference: LinkModePreference.dynamic,
          macOS: os == OS.macOS ? MacOSCodeConfig(targetVersion: 13) : null,
        ),
      ],
      check: (input, output) {
        final codeAssets = output.assets.encodedAssets
            .where((asset) => asset.isCodeAsset)
            .map((asset) => asset.asCodeAsset)
            .toList();
        expect(codeAssets, hasLength(1));
        final asset = codeAssets.single;
        expect(asset.id, 'package:slint/${build_hook.assetName}');
        expect(asset.linkMode, isA<DynamicLoadingBundled>());
        expect(asset.file, isNotNull);
        expect(File.fromUri(asset.file!).existsSync(), isTrue);
        expect(asset.file!.pathSegments.last,
            '${build_hook.assetName}.${build_hook.libraryExtension(os)}');

        // The crate manifest and sources are declared so the hook cache
        // invalidates when they change.
        final dependencies = output.dependencies.map((uri) => uri.path).toSet();
        expect(dependencies, contains(endsWith('api/flutter/Cargo.toml')));
        expect(dependencies, contains(endsWith('api/flutter/rust')));
        expect(dependencies, contains(endsWith('Cargo.lock')));
      },
    );
  });
}
