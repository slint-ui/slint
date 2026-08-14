// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:build/build.dart';
import 'package:slint/slint.dart' show SlintException;
import 'package:slint_generator/builder.dart' as slint_builder;
import 'package:slint_generator/src/builder.dart';
import 'package:test/test.dart';

void main() {
  test('writes generated source and watches compiler dependencies', () async {
    final root = Directory.systemTemp.createTempSync('slint-builder-test');
    addTearDown(() => root.deleteSync(recursive: true));
    final input = AssetId('example', 'lib/main.slint');
    final output = AssetId('example', 'lib/main.slint.dart');
    final dependency = '${root.path}${Platform.pathSeparator}lib'
        '${Platform.pathSeparator}shared.slint';
    final resource = '${root.path}${Platform.pathSeparator}images'
        '${Platform.pathSeparator}logo.png';
    final missing = '${root.path}${Platform.pathSeparator}lib'
        '${Platform.pathSeparator}missing.slint';
    final step = _BuildStep(
      input: input,
      output: output,
      readable: {
        input,
        AssetId('example', 'lib/shared.slint'),
        AssetId('example', 'images/logo.png'),
      },
    );
    final builder = SlintBuilder(
      packageRoot: root.path,
      generator: (inputPath, outputPath, optionsJson) {
        expect(inputPath, endsWith('lib${Platform.pathSeparator}main.slint'));
        expect(
          outputPath,
          endsWith('lib${Platform.pathSeparator}main.slint.dart'),
        );
        expect(jsonDecode(optionsJson), {'include_paths': <Object?>[]});
        return {
          'source': '// generated\n',
          'dependencies': [dependency, resource, missing, dependency],
        };
      },
    );

    await builder.build(step);

    expect(
      step.reads,
      [
        input,
        AssetId('example', 'lib/shared.slint'),
        AssetId('example', 'images/logo.png'),
      ],
    );
    expect(step.checked, contains(AssetId('example', 'lib/missing.slint')));
    expect(step.writes, {output: '// generated\n'});
  });

  test('registers dependencies before reporting a generation error', () async {
    final root = Directory.systemTemp.createTempSync('slint-builder-test');
    addTearDown(() => root.deleteSync(recursive: true));
    final input = AssetId('example', 'lib/main.slint');
    final output = AssetId('example', 'lib/main.slint.dart');
    final dependency = '${root.path}${Platform.pathSeparator}lib'
        '${Platform.pathSeparator}shared.slint';
    final dependencyId = AssetId('example', 'lib/shared.slint');
    final step = _BuildStep(
      input: input,
      output: output,
      readable: {input, dependencyId},
    );
    final builder = SlintBuilder(
      packageRoot: root.path,
      generator: (_, __, ___) => {
        'source': null,
        'error': 'shared.slint does not compile',
        'dependencies': [dependency],
        'diagnostics': [
          {
            'level': 'error',
            'message': 'Unexpected token',
            'file': dependency,
            'line': 3,
            'column': 7,
          },
        ],
      },
    );

    await expectLater(
      builder.build(step),
      throwsA(
        isA<SlintException>().having(
          (error) => error.diagnostics.single.message,
          'structured diagnostic',
          'Unexpected token',
        ),
      ),
    );

    expect(step.reads, [input, dependencyId]);
    expect(step.writes, isEmpty);
  });

  test('logs compiler warning diagnostics', () async {
    final root = Directory.systemTemp.createTempSync('slint-builder-test');
    addTearDown(() => root.deleteSync(recursive: true));
    final input = AssetId('example', 'lib/main.slint');
    final output = AssetId('example', 'lib/main.slint.dart');
    final step = _BuildStep(input: input, output: output, readable: {input});
    final warnings = <String>[];
    final builder = SlintBuilder(
      packageRoot: root.path,
      warningLogger: warnings.add,
      generator: (_, __, ___) => {
        'source': '// generated\n',
        'dependencies': <Object?>[],
        'diagnostics': [
          {
            'level': 'warning',
            'message': 'Deprecated syntax',
            'file': 'lib/main.slint',
            'line': 4,
            'column': 2,
          },
          {
            'level': 'error',
            'message': 'Not logged as a warning',
            'file': null,
            'line': 0,
            'column': 0,
          },
        ],
      },
    );

    await builder.build(step);

    expect(warnings, ['lib/main.slint:4:2: warning: Deprecated syntax']);
  });

  test('rejects dependencies outside the package', () async {
    final root = Directory.systemTemp.createTempSync('slint-builder-test');
    final outside = Directory.systemTemp.createTempSync('slint-outside-test');
    addTearDown(() => root.deleteSync(recursive: true));
    addTearDown(() => outside.deleteSync(recursive: true));
    final input = AssetId('example', 'lib/main.slint');
    final output = AssetId('example', 'lib/main.slint.dart');
    final step = _BuildStep(input: input, output: output, readable: {input});
    final builder = SlintBuilder(
      packageRoot: root.path,
      generator: (_, __, ___) => {
        'source': '// generated\n',
        'dependencies': ['${outside.path}/shared.slint'],
      },
    );

    await expectLater(
      builder.build(step),
      throwsA(
        isA<StateError>().having(
          (error) => error.message,
          'message',
          contains('must be inside'),
        ),
      ),
    );

    expect(step.reads, [input]);
    expect(step.checked, isEmpty);
    expect(step.writes, isEmpty);
  });

  test('passes normalized include paths and style to the generator', () async {
    final root = Directory.systemTemp.createTempSync('slint-builder-test');
    addTearDown(() => root.deleteSync(recursive: true));
    final input = AssetId('example', 'lib/main.slint');
    final output = AssetId('example', 'lib/main.slint.dart');
    final step = _BuildStep(input: input, output: output, readable: {input});
    final builder = SlintBuilder(
      packageRoot: root.path,
      options: {
        'style': 'material',
        'include_paths': ['lib/includes', '${root.path}/lib/shared'],
      },
      generator: (_, __, optionsJson) {
        expect(jsonDecode(optionsJson), {
          'style': 'material',
          'include_paths': [
            '${root.path}${Platform.pathSeparator}lib'
                '${Platform.pathSeparator}includes',
            '${root.path}${Platform.pathSeparator}lib'
                '${Platform.pathSeparator}shared',
          ],
        });
        return {
          'source': '// generated\n',
          'dependencies': <Object?>[],
        };
      },
    );

    await builder.build(step);

    expect(step.writes, {output: '// generated\n'});
  });

  test('public factory validates builder option types', () {
    expect(
      () => slint_builder.slintBuilder(
        const BuilderOptions({'style': 42}),
      ),
      throwsArgumentError,
    );
    expect(
      () => slint_builder.slintBuilder(
        const BuilderOptions({
          'include_paths': ['lib/ui', 42],
        }),
      ),
      throwsArgumentError,
    );
    expect(
      () => slint_builder.slintBuilder(
        const BuilderOptions({'unknown': true}),
      ),
      throwsArgumentError,
    );
  });
}

// ignore: subtype_of_sealed_class
class _BuildStep implements BuildStep {
  _BuildStep({
    required AssetId input,
    required AssetId output,
    required Set<AssetId> readable,
  })  : inputId = input,
        allowedOutputs = [output],
        _readable = readable;

  final Set<AssetId> _readable;
  final List<AssetId> reads = [];
  final List<AssetId> checked = [];
  final Map<AssetId, String> writes = {};

  @override
  final AssetId inputId;

  @override
  final Iterable<AssetId> allowedOutputs;

  @override
  Future<bool> canRead(AssetId id) async {
    checked.add(id);
    return _readable.contains(id);
  }

  @override
  Future<List<int>> readAsBytes(AssetId id) async {
    reads.add(id);
    return const [];
  }

  @override
  Future<void> writeAsString(
    AssetId id,
    FutureOr<String> contents, {
    Encoding encoding = utf8,
  }) async {
    writes[id] = await Future.value(contents);
  }

  @override
  dynamic noSuchMethod(Invocation invocation) => super.noSuchMethod(invocation);
}
