// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/// The compiler behind the code generator.
///
/// `package:slint_generator` calls this from its `build_runner` builder;
/// applications use the generated wrappers instead. It lives here because the
/// generator and the runtime share one native library, and this package is
/// what finds and loads it.
library;

import 'src/ffi.dart';

/// Compile the `.slint` file at [inputPath] into Dart source for [outputPath],
/// configured by [optionsJson].
///
/// Returns the compiler's `source`, its `dependencies`, and any `diagnostics`
/// or generation `error`, all as plain JSON values.
Map<String, Object?> generate(
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
