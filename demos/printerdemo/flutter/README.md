# Printer demo, in Dart

The printer demo driven from Dart, shown inside a Flutter application through
[`slint_flutter`](../../../api/flutter/slint_flutter). It uses the same
[`printerdemo.slint`](../ui/printerdemo.slint) as the Rust, C++, Node.js and
Python versions.

Build the native library, generate the platform runner for the platform you
want, and run:

```sh
cargo build --release -p slint-dart
fvm flutter create --platforms=macos --project-name=printerdemo .
fvm flutter run -d macos
```

Use `linux` or `windows` in place of `macos` as needed. The runner directories
are generated, not committed.

The print queue lives in Dart: `PrinterQueue.printer-queue` is the view of it,
`start-job`, `cancel-job` and `pause-job` mutate it, and a one-second timer
advances the job at the head of the queue.
