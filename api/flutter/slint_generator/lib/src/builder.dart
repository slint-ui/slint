// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import 'dart:convert';
import 'dart:io';

import 'package:build/build.dart';
import 'package:path/path.dart' as p;
import 'package:slint/compiler.dart' as compiler;
import 'package:slint/slint.dart' show Diagnostic, SlintException;

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
      generator: generator ?? compiler.generate,
      warningLogger: warningLogger ?? ((message) => log.warning(message)),
      packageRoot: root,
      outputDir: parsed.outputDir,
      optionsJson: jsonEncode(parsed.compilerOptions),
    );
  }

  SlintBuilder._({
    required SlintGenerator generator,
    required SlintWarningLogger warningLogger,
    required String packageRoot,
    required String outputDir,
    required String optionsJson,
  })  : _generator = generator,
        _warningLogger = warningLogger,
        _packageRoot = packageRoot,
        _outputDir = outputDir,
        _optionsJson = optionsJson;

  final SlintGenerator _generator;
  final SlintWarningLogger _warningLogger;
  final String _packageRoot;
  final String _outputDir;
  final String _optionsJson;

  @override
  Map<String, List<String>> get buildExtensions {
    if (_outputDir.isEmpty) {
      return const {
        '.slint': ['.slint.dart']
      };
    }
    // The capture group mirrors each source's path relative to the package's
    // `lib` directory into [outputDir], so `lib/ui/counter.slint` becomes
    // `<outputDir>/ui/counter.slint.dart`.
    return {
      'lib/{{path}}.slint': ['$_outputDir/{{path}}.slint.dart']
    };
  }

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

  static String _packageRelativeOutputDir(String value, String packageRoot) {
    final normalized = p.normalize(
      p.isAbsolute(value) ? value : p.join(packageRoot, value),
    );
    if (!p.isWithin(packageRoot, normalized)) {
      throw ArgumentError.value(
        value,
        'output_dir',
        'Must be inside the package',
      );
    }
    return p.url.joinAll(p.split(p.relative(normalized, from: packageRoot)));
  }

  static ({String outputDir, Map<String, Object?> compilerOptions})
      _parseOptions(
    Map<String, dynamic> options,
    String packageRoot,
  ) {
    const supported = {'include_paths', 'style', 'output_dir'};
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

    final outputDirValue = options['output_dir'];
    if (outputDirValue != null && outputDirValue is! String) {
      throw ArgumentError.value(
          outputDirValue, 'output_dir', 'Must be a string');
    }
    final String? outputDirString = outputDirValue as String?;
    final outputDir = outputDirString == null
        ? ''
        : _packageRelativeOutputDir(outputDirString, packageRoot);

    return (
      outputDir: outputDir,
      compilerOptions: {
        if (styleValue != null) 'style': styleValue,
        'include_paths': includePaths,
      },
    );
  }
}
