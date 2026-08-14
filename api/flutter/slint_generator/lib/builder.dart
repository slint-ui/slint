// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/// Code generation support for `package:build`.
library;

import 'package:build/build.dart';

import 'src/builder.dart';

/// Creates the builder that generates a typed Dart API for each `.slint` file.
Builder slintBuilder(BuilderOptions options) =>
    SlintBuilder(options: options.config);
