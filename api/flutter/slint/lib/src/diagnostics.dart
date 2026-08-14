// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/// Errors and compiler diagnostics.
library;

/// A message the Slint compiler produced while compiling a `.slint` file.
class Diagnostic {
  Diagnostic({
    required this.level,
    required this.message,
    this.file,
    this.line = 0,
    this.column = 0,
  });

  Diagnostic.fromJson(Map<String, dynamic> json)
      : level = json['level'] as String,
        message = json['message'] as String,
        file = json['file'] as String?,
        line = json['line'] as int,
        column = json['column'] as int;

  /// Either `error` or `warning`.
  final String level;
  final String message;
  final String? file;

  /// One-based, or 0 when the diagnostic has no source location.
  final int line;

  /// One-based, or 0 when the diagnostic has no source location.
  final int column;

  bool get isError => level == 'error';

  @override
  String toString() {
    final location = file == null ? '' : '$file:$line:$column: ';
    return '$location$level: $message';
  }
}

/// Thrown when Slint rejects something: a `.slint` file that does not compile,
/// an unknown property or callback, or a value of the wrong type.
class SlintException implements Exception {
  SlintException(this.message, [this.diagnostics = const []]);

  final String message;

  /// The compiler diagnostics, when this exception comes from a failed
  /// compilation. Empty otherwise.
  final List<Diagnostic> diagnostics;

  @override
  String toString() => [
        'SlintException: $message',
        ...diagnostics.map((d) => '  $d')
      ].join('\n');
}
