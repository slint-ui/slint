// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import 'package:slint_codegen_example/ui/counter.slint.dart';

void main() {
  final app = CounterWindow.load()
    ..statusMessage = 'Click the window'
    ..onCountChanged((value) => print('Count: $value'));

  app.currentCount = 3;
  app.invokeResetCounter();
  app.run();
}
