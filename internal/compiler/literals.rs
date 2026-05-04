// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Span, Spanned};
use crate::expression_tree::Expression;
use crate::expression_tree::Unit;
use itertools::Itertools;
use smol_str::SmolStr;
use strum::IntoEnumIterator;

/// Describes one chunk produced by [`walk_escapes`].
enum EscapeChunk<'a> {
    /// Consecutive plain characters (same bytes in source and output).
    Plain(&'a str),
    /// An escape sequence: `source_len` bytes in the source produce `decoded`.
    Escape { source_len: usize, decoded: char },
}

/// Error returned by [`walk_escapes`]: byte offset within the raw token and a
/// human-readable message.
struct EscapeError {
    offset: usize,
    length: usize,
    message: &'static str,
}

/// Walk a string literal token (including its delimiters), strip the delimiters,
/// and call `callback` for each chunk of the content. Returns `Ok(())` on success,
/// or an [`EscapeError`] pointing at the problematic byte in the raw token.
fn walk_escapes<'a>(
    raw_token: &'a str,
    mut callback: impl FnMut(EscapeChunk<'a>),
) -> Result<(), EscapeError> {
    if raw_token.contains('\n') {
        return Err(EscapeError { offset: 0, length: 0, message: "Newline in string literal" });
    }
    let prefix_len = if raw_token.starts_with('"') || raw_token.starts_with('}') {
        1
    } else {
        return Err(EscapeError { offset: 0, length: 0, message: "Cannot parse string literal" });
    };
    let content = &raw_token[prefix_len..];
    let content = content
        .strip_suffix('"')
        .or_else(|| content.strip_suffix("\\{"))
        .ok_or(EscapeError { offset: 0, length: 0, message: "Cannot parse string literal" })?;

    let mut pos = 0;
    while pos < content.len() {
        if content.as_bytes()[pos] == b'\\' {
            if pos + 1 >= content.len() {
                return Err(EscapeError {
                    offset: prefix_len + pos,
                    length: 1,
                    message: r"Unknown escape sequence. Use '\\' to escape a literal backslash",
                });
            }
            let (source_len, decoded) = match content.as_bytes()[pos + 1] {
                b'"' => (2, '"'),
                b'\\' => (2, '\\'),
                b'n' => (2, '\n'),
                b'u' => {
                    let brace_start = pos + 2;
                    let has_brace = content.as_bytes().get(brace_start) == Some(&b'{');
                    if !has_brace {
                        return Err(EscapeError {
                            offset: prefix_len + brace_start,
                            length: 0,
                            message: "Invalid unicode escape: expected '{'",
                        });
                    }
                    let brace_end = match content[brace_start..].find('}') {
                        Some(i) => i + brace_start,
                        None => {
                            return Err(EscapeError {
                                offset: prefix_len + brace_start,
                                length: 0,
                                message: "Unterminated unicode escape",
                            });
                        }
                    };
                    let hex = &content[brace_start + 1..brace_end];
                    let x = u32::from_str_radix(hex, 16).map_err(|_| EscapeError {
                        offset: prefix_len + brace_start + 1,
                        length: hex.len(),
                        message: "Invalid hexadecimal in unicode escape",
                    })?;
                    let ch = std::char::from_u32(x).ok_or(EscapeError {
                        offset: prefix_len + brace_start + 1,
                        length: hex.len(),
                        message: "Invalid unicode code point",
                    })?;
                    (brace_end + 1 - pos, ch)
                }
                _ => {
                    let next_char_len =
                        content[pos + 1..].chars().next().map_or(1, |c| c.len_utf8());
                    return Err(EscapeError {
                        offset: prefix_len + pos,
                        length: 1 + next_char_len,
                        message: r"Unknown escape sequence. Use '\\' to escape a literal backslash",
                    });
                }
            };
            callback(EscapeChunk::Escape { source_len, decoded });
            pos += source_len;
        } else {
            let start = pos;
            pos = content[pos..].find('\\').map_or(content.len(), |i| pos + i);
            callback(EscapeChunk::Plain(&content[start..pos]));
        }
    }
    Ok(())
}

/// Unescape a string literal token, returning `None` on error.
pub fn unescape_string(string: &str) -> Option<SmolStr> {
    let mut result = String::with_capacity(string.len());
    walk_escapes(string, |chunk| match chunk {
        EscapeChunk::Plain(s) => result += s,
        EscapeChunk::Escape { decoded, .. } => result.push(decoded),
    })
    .ok()?;
    Some(result.into())
}

/// Unescape a string literal token, reporting any error on the token's source location
/// with the span pointing at the invalid escape sequence.
/// If `token` is `None` (no string literal found), reports a generic error on `fallback`.
pub fn unescape_string_reporting(
    token: Option<&crate::parser::SyntaxToken>,
    diag: &mut BuildDiagnostics,
    fallback: &dyn Spanned,
) -> Option<SmolStr> {
    let Some(token) = token else {
        diag.push_error("Cannot parse string literal".into(), fallback);
        return None;
    };
    let mut result = String::with_capacity(token.text().len());
    match walk_escapes(token.text(), |chunk| match chunk {
        EscapeChunk::Plain(s) => result += s,
        EscapeChunk::Escape { decoded, .. } => result.push(decoded),
    }) {
        Ok(()) => Some(result.into()),
        Err(e) => {
            let loc = token.to_source_location();
            diag.push_error_with_span(
                e.message.into(),
                SourceLocation {
                    source_file: loc.source_file,
                    span: Span::new(loc.span.offset + e.offset, e.length),
                },
            );
            None
        }
    }
}

#[test]
fn test_unescape_string() {
    assert_eq!(unescape_string(r#""foo_bar""#).as_deref(), Some("foo_bar"));
    assert_eq!(unescape_string(r#""foo\"bar""#).as_deref(), Some("foo\"bar"));
    assert_eq!(unescape_string(r#""foo\\\"bar""#).as_deref(), Some("foo\\\"bar"));
    assert_eq!(unescape_string(r#""fo\na\\r""#).as_deref(), Some("fo\na\\r"));
    assert_eq!(unescape_string(r#""fo\xa""#), None);
    assert_eq!(unescape_string(r#""fooo\""#), None);
    assert_eq!(unescape_string(r#""f\n\n\nf""#).as_deref(), Some("f\n\n\nf"));
    assert_eq!(unescape_string(r#""music\♪xx""#), None);
    assert_eq!(unescape_string(r#""music\"♪\"🎝""#).as_deref(), Some("music\"♪\"🎝"));
    assert_eq!(unescape_string(r#""foo_bar"#), None);
    assert_eq!(unescape_string(r#""foo_bar\"#), None);
    assert_eq!(unescape_string(r#"foo_bar""#), None);
    assert_eq!(
        unescape_string(r#""d\u{8}a\u{d4}f\u{Ed3}""#).as_deref(),
        Some("d\u{8}a\u{d4}f\u{ED3}")
    );
    assert_eq!(unescape_string(r#""xxx\""#), None);
    assert_eq!(unescape_string(r#""xxx\u""#), None);
    assert_eq!(unescape_string(r#""xxx\uxx""#), None);
    assert_eq!(unescape_string(r#""xxx\u{""#), None);
    assert_eq!(unescape_string(r#""xxx\u{22""#), None);
    assert_eq!(unescape_string(r#""xxx\u{qsdf}""#), None);
    assert_eq!(unescape_string(r#""xxx\u{1234567890}""#), None);
}

/// Maps byte offsets in a string assembled from one or more string literal tokens
/// back to precise source locations, accounting for escape sequences.
#[derive(Default)]
pub struct StringLiteralSourceMap {
    assembled: String,
    entries: Vec<SourceMapEntry>,
}

/// One segment where assembled-string offsets map 1:1 to source-file offsets.
/// A new entry is created at every escape boundary.
struct SourceMapEntry {
    /// Start byte offset in the assembled (unescaped) string.
    assembled_start: usize,
    /// Absolute byte offset in the source file corresponding to `assembled_start`.
    source_offset: usize,
    source_file: Option<crate::diagnostics::SourceFile>,
}

impl StringLiteralSourceMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the assembled (unescaped) string.
    pub fn as_str(&self) -> &str {
        &self.assembled
    }

    /// Consume the source map and return the assembled string.
    pub fn into_string(self) -> String {
        self.assembled
    }

    /// Unescape a string literal token, appending to the internal assembled string
    /// and recording the source mapping. Reports errors to `diag` and returns
    /// `false` on failure.
    pub fn push(
        &mut self,
        token: &crate::parser::SyntaxToken,
        diag: &mut BuildDiagnostics,
    ) -> bool {
        let loc = token.to_source_location();
        let token_offset = loc.span.offset;
        let raw = token.text();
        let base = self.assembled.len();

        let mut source_pos = 1usize;
        let mut segment_start_assembled = base;
        let mut segment_start_source = 1usize;

        let result = walk_escapes(raw, |chunk| match chunk {
            EscapeChunk::Plain(s) => {
                self.assembled += s;
                source_pos += s.len();
            }
            EscapeChunk::Escape { source_len, decoded } => {
                if self.assembled.len() > segment_start_assembled {
                    self.entries.push(SourceMapEntry {
                        assembled_start: segment_start_assembled,
                        source_offset: token_offset + segment_start_source,
                        source_file: loc.source_file.clone(),
                    });
                }
                self.entries.push(SourceMapEntry {
                    assembled_start: self.assembled.len(),
                    source_offset: token_offset + source_pos,
                    source_file: loc.source_file.clone(),
                });
                self.assembled.push(decoded);
                source_pos += source_len;
                segment_start_assembled = self.assembled.len();
                segment_start_source = source_pos;
            }
        });

        match result {
            Ok(()) => {
                if self.assembled.len() > segment_start_assembled {
                    self.entries.push(SourceMapEntry {
                        assembled_start: segment_start_assembled,
                        source_offset: token_offset + segment_start_source,
                        source_file: loc.source_file,
                    });
                }
                true
            }
            Err(e) => {
                self.assembled.truncate(base);
                diag.push_error_with_span(
                    e.message.into(),
                    SourceLocation {
                        source_file: loc.source_file,
                        span: Span::new(loc.span.offset + e.offset, e.length),
                    },
                );
                false
            }
        }
    }

    /// Append a non-literal character (e.g., an interpolation placeholder)
    /// where source and assembled offsets correspond 1:1.
    pub fn push_raw_char(&mut self, ch: char, loc: SourceLocation) {
        let start = self.assembled.len();
        self.assembled.push(ch);
        self.entries.push(SourceMapEntry {
            assembled_start: start,
            source_offset: loc.span.offset,
            source_file: loc.source_file,
        });
    }

    /// Resolve a byte range in the assembled string to a precise source location.
    /// The returned span points at the specific position within the string literal.
    pub fn resolve(&self, range: std::ops::Range<usize>) -> Option<SourceLocation> {
        let idx = self.entries.partition_point(|e| e.assembled_start <= range.start);
        if idx == 0 {
            return None;
        }
        let entry = &self.entries[idx - 1];
        let delta = range.start - entry.assembled_start;
        Some(SourceLocation {
            source_file: entry.source_file.clone(),
            span: Span::new(entry.source_offset + delta, range.len()),
        })
    }

    /// Report an error at a precise position within the string, falling back to
    /// the full node if the position cannot be resolved.
    pub fn report(
        &self,
        diag: &mut BuildDiagnostics,
        message: String,
        range: std::ops::Range<usize>,
        fallback: &dyn Spanned,
    ) {
        if let Some(loc) = self.resolve(range) {
            diag.push_error_with_span(message, loc);
        } else {
            diag.push_error(message, fallback);
        }
    }
}

pub fn parse_number_literal(s: SmolStr) -> Result<Expression, SmolStr> {
    let bytes = s.as_bytes();
    let mut end = 0;
    while end < bytes.len() && matches!(bytes[end], b'0'..=b'9' | b'.') {
        end += 1;
    }
    let val = s[..end].parse().map_err(|_| "Cannot parse number literal".to_owned())?;
    let unit = s[end..].parse().map_err(|_| {
        format!(
            "Invalid unit '{}'. Valid units are: {}",
            s.get(end..).unwrap_or(&s),
            Unit::iter().filter(|x| !x.to_string().is_empty()).join(", ")
        )
    })?;
    Ok(Expression::NumberLiteral(val, unit))
}

#[test]
fn test_parse_number_literal() {
    use crate::expression_tree::Unit;
    use smol_str::{ToSmolStr, format_smolstr};

    fn doit(s: &str) -> Result<(f64, Unit), SmolStr> {
        parse_number_literal(s.into()).map(|e| match e {
            Expression::NumberLiteral(a, b) => (a, b),
            _ => panic!(),
        })
    }

    assert_eq!(doit("10"), Ok((10., Unit::None)));
    assert_eq!(doit("10phx"), Ok((10., Unit::Phx)));
    assert_eq!(doit("10.0phx"), Ok((10., Unit::Phx)));
    assert_eq!(doit("10.0"), Ok((10., Unit::None)));
    assert_eq!(doit("1.1phx"), Ok((1.1, Unit::Phx)));
    assert_eq!(doit("10.10"), Ok((10.10, Unit::None)));
    assert_eq!(doit("10000000"), Ok((10000000., Unit::None)));
    assert_eq!(doit("10000001phx"), Ok((10000001., Unit::Phx)));

    let cannot_parse = Err("Cannot parse number literal".to_smolstr());
    assert_eq!(doit("12.10.12phx"), cannot_parse);

    let valid_units = Unit::iter().filter(|x| !x.to_string().is_empty()).join(", ");
    let wrong_unit_spaced =
        Err(format_smolstr!("Invalid unit ' phx'. Valid units are: {}", valid_units));
    assert_eq!(doit("10000001 phx"), wrong_unit_spaced);
    let wrong_unit_oo = Err(format_smolstr!("Invalid unit 'oo'. Valid units are: {}", valid_units));
    assert_eq!(doit("12.12oo"), wrong_unit_oo);
    let wrong_unit_euro =
        Err(format_smolstr!("Invalid unit '€'. Valid units are: {}", valid_units));
    assert_eq!(doit("12.12€"), wrong_unit_euro);
}
