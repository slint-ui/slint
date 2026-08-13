// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import 'dart:convert';
import 'dart:io';

import 'package:build/build.dart';
import 'package:path/path.dart' as p;

import 'diagnostics.dart';
import 'ffi.dart';

typedef SlintGenerator = Map<String, Object?> Function(
  String inputPath,
  String outputPath,
  String optionsJson,
);

typedef SlintWarningLogger = void Function(String message);

/// Generates `.slint.dart` files and registers every compiler dependency.
///
/// The optional arguments make the filesystem boundary testable without
/// loading the native library. They aren't part of the public package API.
class SlintBuilder implements Builder {
  factory SlintBuilder({
    SlintGenerator? generator,
    SlintWarningLogger? warningLogger,
    String? packageRoot,
    Map<String, dynamic> options = const {},
  }) {
    final root = p.normalize(p.absolute(packageRoot ?? Directory.current.path));
    final parsed = _parseOptions(options, root);
    return SlintBuilder._(
      generator: generator ?? _generate,
      warningLogger: warningLogger ?? ((message) => log.warning(message)),
      packageRoot: root,
      optionsJson: jsonEncode(parsed),
    );
  }

  SlintBuilder._({
    required SlintGenerator generator,
    required SlintWarningLogger warningLogger,
    required String packageRoot,
    required String optionsJson,
  })  : _generator = generator,
        _warningLogger = warningLogger,
        _packageRoot = packageRoot,
        _optionsJson = optionsJson;

  static const _buildExtensions = <String, List<String>>{
    '.slint': ['.slint.dart'],
  };

  final SlintGenerator _generator;
  final SlintWarningLogger _warningLogger;
  final String _packageRoot;
  final String _optionsJson;

  @override
  Map<String, List<String>> get buildExtensions => _buildExtensions;

  @override
  Future<void> build(BuildStep buildStep) async {
    // The compiler reads from the filesystem, but this read tells build_runner
    // that the primary input affects the generated output.
    await buildStep.readAsBytes(buildStep.inputId);

    final outputId = buildStep.allowedOutputs.single;
    final inputPath = _filePath(buildStep.inputId.path);
    final outputPath = _filePath(outputId.path);
    final result = _generator(inputPath, outputPath, _optionsJson);
    final source = result['source'];
    final generationError = result['error'];
    final dependencies = result['dependencies'];
    final rawDiagnostics = result['diagnostics'];
    if (source != null && source is! String ||
        generationError != null && generationError is! String ||
        dependencies is! List<Object?> ||
        rawDiagnostics != null && rawDiagnostics is! List<Object?>) {
      throw StateError(
          'The Slint compiler returned an invalid generation result.');
    }
    final generatedSource = source as String?;
    final error = generationError as String?;
    final diagnostics =
        (rawDiagnostics as List<Object?>? ?? const []).map((diagnostic) {
      if (diagnostic is! Map<String, dynamic>) {
        throw StateError('The Slint compiler returned an invalid diagnostic.');
      }
      return Diagnostic.fromJson(diagnostic);
    }).toList();

    final seen = <AssetId>{buildStep.inputId};
    final watchedDependencies = <AssetId>[];
    final externalDependencies = <String>[];
    for (final dependency in dependencies) {
      if (dependency is! String) {
        throw StateError(
            'The Slint compiler returned a non-string dependency.');
      }
      final dependencyId = _dependencyAssetId(
        dependency,
        package: buildStep.inputId.package,
      );
      if (dependencyId == null) {
        externalDependencies.add(dependency);
        continue;
      }
      if (!seen.add(dependencyId)) continue;
      watchedDependencies.add(dependencyId);
    }

    for (final dependencyId in watchedDependencies) {
      // A read records an existing dependency in build_runner's asset graph.
      if (await buildStep.canRead(dependencyId)) {
        await buildStep.readAsBytes(dependencyId);
      }
    }

    for (final diagnostic
        in diagnostics.where((diagnostic) => !diagnostic.isError)) {
      _warningLogger(diagnostic.toString());
    }

    if (externalDependencies.isNotEmpty) {
      throw StateError(
        'Slint dependencies must be inside $_packageRoot so build_runner can '
        'watch them:\n${externalDependencies.join('\n')}',
      );
    }
    if (error != null) {
      throw SlintException(error, diagnostics);
    }
    if (generatedSource == null) {
      throw StateError('The Slint compiler returned no generated source.');
    }
    await buildStep.writeAsString(outputId, generatedSource);
  }

  String _filePath(String assetPath) =>
      p.join(_packageRoot, p.joinAll(p.url.split(assetPath)));

  AssetId? _dependencyAssetId(String dependency, {required String package}) {
    final absolutePath = p.normalize(
      p.isAbsolute(dependency) ? dependency : p.join(_packageRoot, dependency),
    );
    if (!p.isWithin(_packageRoot, absolutePath)) return null;
    final relativePath = p.relative(absolutePath, from: _packageRoot);
    return AssetId(package, p.url.joinAll(p.split(relativePath)));
  }

  static Map<String, Object?> _parseOptions(
    Map<String, dynamic> options,
    String packageRoot,
  ) {
    const supported = {'include_paths', 'style'};
    for (final name in options.keys) {
      if (!supported.contains(name)) {
        throw ArgumentError.value(
            name, 'options', 'Unknown Slint builder option');
      }
    }

    final styleValue = options['style'];
    if (options.containsKey('style') && styleValue is! String) {
      throw ArgumentError.value(styleValue, 'style', 'Must be a string');
    }

    final includePathsValue = options['include_paths'];
    if (options.containsKey('include_paths') &&
        includePathsValue is! List<Object?>) {
      throw ArgumentError.value(
        includePathsValue,
        'include_paths',
        'Must be a list of strings',
      );
    }
    final includePaths = <String>[];
    for (final value in includePathsValue as List<Object?>? ?? const []) {
      if (value is! String) {
        throw ArgumentError.value(
          includePathsValue,
          'include_paths',
          'Must be a list of strings',
        );
      }
      includePaths.add(
        p.normalize(p.isAbsolute(value) ? value : p.join(packageRoot, value)),
      );
    }

    return {
      if (styleValue != null) 'style': styleValue,
      'include_paths': includePaths,
    };
  }

  static Map<String, Object?> _generate(
    String inputPath,
    String outputPath,
    String optionsJson,
  ) {
    final pointer = withNativeString(
      inputPath,
      (input) => withNativeString(
        outputPath,
        (output) => withNativeString(
          optionsJson,
          (options) => SlintFfi.instance.generate(input, output, options),
        ),
      ),
    );
    final result = takeEnvelope(pointer);
    if (result is! Map<String, dynamic>) {
      throw StateError('The Slint compiler returned an invalid response.');
    }
    return result.cast<String, Object?>();
  }
}
