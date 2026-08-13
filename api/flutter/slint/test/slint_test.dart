// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import 'dart:io';

import 'package:slint/slint.dart';
import 'package:test/test.dart';

const _app = '''
export struct TodoItem { title: string, checked: bool }

export global Logic {
    in-out property <int> offset: 3;
    callback shout(string) -> string;
    callback noted();
}

export component App {
    in-out property <int> value: 42;
    in-out property <string> label: "hello";
    in-out property <bool> flag: true;
    in-out property <float> ratio: 0.5;
    in-out property <[TodoItem]> items: [{ title: "a", checked: true }];
    in-out property <TodoItem> current: { title: "b", checked: false };
    callback add(string) -> int;
    public function double(v: int) -> int { v * 2 }
}
''';

void main() {
  late ComponentInstance app;

  setUp(() => app = loadSource(_app));
  tearDown(() => app.dispose());

  group('properties', () {
    test('public property methods preserve operator behavior', () {
      expect(app.getProperty('value'), 42);
      app.setProperty('value', 7);
      expect(app['value'], 7);

      app['value'] = 11;
      expect(app.getProperty('value'), 11);
    });

    test('round-trip scalars', () {
      expect(app['value'], 42);
      expect(app['label'], 'hello');
      expect(app['flag'], true);
      expect(app['ratio'], 0.5);

      app['value'] = 7;
      app['label'] = 'bye';
      app['flag'] = false;
      app['ratio'] = 0.25;

      expect(app['value'], 7);
      expect(app['label'], 'bye');
      expect(app['flag'], false);
      expect(app['ratio'], 0.25);
    });

    test('a struct is a Map', () {
      expect(app['current'], {'title': 'b', 'checked': false});

      app['current'] = {'title': 'c', 'checked': true};
      expect(app['current'], {'title': 'c', 'checked': true});
    });

    test('a model is a List of Maps', () {
      expect(app['items'], [
        {'title': 'a', 'checked': true},
      ]);

      app['items'] = [
        {'title': 'x', 'checked': false},
        {'title': 'y', 'checked': true},
      ];
      expect(app['items'], [
        {'title': 'x', 'checked': false},
        {'title': 'y', 'checked': true},
      ]);
    });

    test('an unknown name throws', () {
      expect(() => app['nope'], throwsA(isA<SlintException>()));
      expect(() => app['nope'] = 1, throwsA(isA<SlintException>()));
    });

    test('a value of the wrong type throws and leaves the property alone', () {
      expect(() => app['value'] = 'not a number',
          throwsA(isA<SlintException>()));
      expect(app['value'], 42);
    });
  });

  group('globals', () {
    test('public property methods preserve operator behavior', () {
      final logic = app.global('Logic');
      expect(logic.getProperty('offset'), 3);
      logic.setProperty('offset', 9);
      expect(logic['offset'], 9);

      logic['offset'] = 12;
      expect(logic.getProperty('offset'), 12);
    });

    test('round-trip a property', () {
      final logic = app.global('Logic');
      expect(logic['offset'], 3);
      logic['offset'] = 9;
      expect(logic['offset'], 9);
    });

    test('an unknown global throws', () {
      expect(() => app.global('Nope')['offset'],
          throwsA(isA<SlintException>()));
    });
  });

  group('callbacks and functions', () {
    test('invoke a public function', () {
      expect(app.invoke('double', [21]), 42);
    });

    test('a handler sees the arguments and its result comes back', () {
      final seen = <List<Object?>>[];
      app.setCallback('add', (args) {
        seen.add(args);
        return (args[0]! as String).length;
      });

      expect(app.invoke('add', ['milk']), 4);
      expect(seen, [
        ['milk'],
      ]);
    });

    test('a handler on a global', () {
      app.global('Logic').setCallback(
            'shout',
            (args) => (args[0]! as String).toUpperCase(),
          );
      expect(app.global('Logic').invoke('shout', ['hello']), 'HELLO');
    });

    test('a void callback may return null', () {
      var called = 0;
      app.global('Logic').setCallback('noted', (_) {
        called++;
        return null;
      });
      app.global('Logic').invoke('noted');
      expect(called, 1);
    });

    test('the argument count is checked', () {
      expect(() => app.invoke('double'), throwsA(isA<SlintException>()));
    });

    test('an unknown callback throws', () {
      expect(() => app.setCallback('nope', (_) => null),
          throwsA(isA<SlintException>()));
    });
  });

  group('loading', () {
    test('a compile error carries diagnostics', () {
      try {
        loadSource('export component Broken { this is not slint }');
        fail('expected a SlintException');
      } on SlintException catch (e) {
        expect(e.diagnostics, isNotEmpty);
        expect(e.diagnostics.first.isError, isTrue);
        expect(e.diagnostics.first.message, isNotEmpty);
        expect(e.toString(), contains('compilation failed'));
      }
    });

    test('an unknown component name throws', () {
      expect(
        () => loadSource(_app, component: 'Nope'),
        throwsA(isA<SlintException>()),
      );
    });

    test('a named component can be picked', () {
      final named = loadSource(_app, component: 'App');
      addTearDown(named.dispose);
      expect(named['value'], 42);
    });

    test('loadFile compiles from disk', () {
      final file = File('${Directory.systemTemp.createTempSync().path}/t.slint')
        ..writeAsStringSync(_app);
      addTearDown(() => file.parent.deleteSync(recursive: true));

      final loaded = loadFile(file.path);
      addTearDown(loaded.dispose);
      expect(loaded['label'], 'hello');
    });

    test('a missing file reports a diagnostic instead of crashing', () {
      expect(
        () => loadFile('/definitely/not/here.slint'),
        throwsA(isA<SlintException>()),
      );
    });
  });

  group('lifetime', () {
    test('a runtime instance is also a SlintComponent', () {
      final SlintComponent component = app;
      expect(identical(component.instance, app), isTrue);
    });

    test('using a disposed instance throws', () {
      final throwaway = loadSource(_app)..dispose();
      expect(() => throwaway['value'], throwsA(isA<StateError>()));
      // Disposing twice is harmless.
      throwaway.dispose();
    });
  });
}
