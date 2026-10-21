// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use proc_macro2::TokenStream;

/// Writes `code`, as returned by [`super::rust::generate`], to `sink` as Rust source code.
///
/// rustc is slow on very long lines: it computes the column of every panic location from the
/// start of its line. So the code gets a line break after each `{`, `}`, and `;`.
pub fn write_formatted(code: &TokenStream, sink: impl std::io::Write) -> std::io::Result<()> {
    use std::io::Write;
    CodeFormatter::new(sink).write_all(code.to_string().as_bytes())
}

struct CodeFormatter<Sink> {
    indentation: usize,
    /// We are currently in a string
    in_string: bool,
    /// number of bytes after the last `'`, 0 if there was none
    in_char: usize,
    /// In string or char, and the previous character was `\\`
    escaped: bool,
    sink: Sink,
}

impl<Sink> CodeFormatter<Sink> {
    fn new(sink: Sink) -> Self {
        Self { indentation: 0, in_string: false, in_char: 0, escaped: false, sink }
    }
}

impl<Sink: std::io::Write> std::io::Write for CodeFormatter<Sink> {
    fn write(&mut self, mut s: &[u8]) -> std::io::Result<usize> {
        let len = s.len();
        while let Some(idx) = s.iter().position(|c| match c {
            b'{' if !self.in_string && self.in_char == 0 => {
                self.indentation += 1;
                true
            }
            b'}' if !self.in_string && self.in_char == 0 => {
                self.indentation -= 1;
                true
            }
            b';' if !self.in_string && self.in_char == 0 => true,
            b'"' if !self.in_string && self.in_char == 0 => {
                self.in_string = true;
                self.escaped = false;
                false
            }
            b'"' if self.in_string && !self.escaped => {
                self.in_string = false;
                false
            }
            b'\'' if !self.in_string && self.in_char == 0 => {
                self.in_char = 1;
                self.escaped = false;
                false
            }
            b'\'' if !self.in_string && self.in_char > 0 && !self.escaped => {
                self.in_char = 0;
                false
            }
            b' ' | b'>' if self.in_char > 2 && !self.escaped => {
                // probably a lifetime
                self.in_char = 0;
                false
            }
            b'\\' if (self.in_string || self.in_char > 0) && !self.escaped => {
                self.escaped = true;
                // no need to increment in_char since \ isn't a single character
                false
            }
            _ if self.in_char > 0 => {
                self.in_char += 1;
                self.escaped = false;
                false
            }
            _ => {
                self.escaped = false;
                false
            }
        }) {
            let idx = idx + 1;
            self.sink.write_all(&s[..idx])?;
            self.sink.write_all(b"\n")?;
            for _ in 0..self.indentation {
                self.sink.write_all(b"    ")?;
            }
            s = &s[idx..];
        }
        self.sink.write_all(s)?;
        Ok(len)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.sink.flush()
    }
}

#[test]
fn formatter_test() {
    use std::io::Write;
    fn format_code(code: &str) -> String {
        let mut res = Vec::new();
        let mut formatter = CodeFormatter::new(&mut res);
        formatter.write_all(code.as_bytes()).unwrap();
        String::from_utf8(res).unwrap()
    }

    assert_eq!(
        format_code("fn main() { if ';' == '}' { return \";\"; } else { panic!() } }"),
        r#"fn main() {
     if ';' == '}' {
         return ";";
         }
     else {
         panic!() }
     }
"#
    );

    assert_eq!(
        format_code(r#"fn xx<'lt>(foo: &'lt str) { println!("{}", '\u{f700}'); return Ok(()); }"#),
        r#"fn xx<'lt>(foo: &'lt str) {
     println!("{}", '\u{f700}');
     return Ok(());
     }
"#
    );

    assert_eq!(
        format_code(r#"fn main() { ""; "'"; "\""; "{}"; "\\"; "\\\""; }"#),
        r#"fn main() {
     "";
     "'";
     "\"";
     "{}";
     "\\";
     "\\\"";
     }
"#
    );

    assert_eq!(
        format_code(r#"fn main() { '"'; '\''; '{'; '}'; '\\'; }"#),
        r#"fn main() {
     '"';
     '\'';
     '{';
     '}';
     '\\';
     }
"#
    );
}
