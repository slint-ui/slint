// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Span, Spanned};
use crate::expression_tree::Expression;
use crate::expression_tree::Unit;
use itertools::Itertools;
use smol_str::SmolStr;
use strum::IntoEnumIterator;

/// Describes one chunk produced by [`walk_escapes`].
enum EscapeChunk {
    /// A plain character (same bytes in source and output).
    Plain { len: usize },
    /// An escape sequence: `source_len` bytes in the source produce `decoded`.
    Escape { source_len: usize, decoded: char },
}

/// Walk the content of a string literal (after stripping the opening delimiter),
/// calling `callback` for each chunk. Returns `Ok(())` on success, or `Err(pos)`
/// with the byte offset of the malformed escape within `content`.
fn walk_escapes(content: &str, mut callback: impl FnMut(EscapeChunk)) -> Result<(), usize> {
    let mut pos = 0;
    while pos < content.len() {
        if content.as_bytes()[pos] == b'\\' {
            if pos + 1 >= content.len() {
                return Err(pos);
            }
            let (source_len, decoded) = match content.as_bytes()[pos + 1] {
                b'"' => (2, '"'),
                b'\\' => (2, '\\'),
                b'n' => (2, '\n'),
                b'u' => {
                    let brace_start = pos + 2;
                    let has_brace = content.as_bytes().get(brace_start) == Some(&b'{');
                    if !has_brace {
                        return Err(pos);
                    }
                    let brace_end = match content[brace_start..].find('}') {
                        Some(i) => i + brace_start,
                        None => return Err(pos),
                    };
                    let hex = &content[brace_start + 1..brace_end];
                    let x = u32::from_str_radix(hex, 16).map_err(|_| pos)?;
                    let ch = std::char::from_u32(x).ok_or(pos)?;
                    (brace_end + 1 - pos, ch)
                }
                _ => return Err(pos),
            };
            callback(EscapeChunk::Escape { source_len, decoded });
            pos += source_len;
        } else {
            let ch_len = content[pos..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            callback(EscapeChunk::Plain { len: ch_len });
            pos += ch_len;
        }
    }
    Ok(())
}

/// Unescape a string literal token. Returns `Ok(unescaped)` on success,
/// or `Err(offset)` with the byte offset of the bad escape within the raw token.
pub fn unescape_string(string: &str) -> Result<SmolStr, usize> {
    if string.contains('\n') {
        // FIXME: new line in string literal not yet supported
        return Err(0);
    }
    let prefix_len =
        if string.starts_with('"') || string.starts_with('}') { 1 } else { return Err(0) };
    let content = &string[prefix_len..];
    let content =
        content.strip_suffix('"').or_else(|| content.strip_suffix("\\{")).ok_or(0usize)?;
    if !content.contains('\\') {
        return Ok(content.into());
    }
    let mut result = String::with_capacity(content.len());
    let mut pos = 0;
    walk_escapes(content, |chunk| match chunk {
        EscapeChunk::Plain { len } => {
            result += &content[pos..pos + len];
            pos += len;
        }
        EscapeChunk::Escape { source_len, decoded } => {
            result.push(decoded);
            pos += source_len;
        }
    })
    // Map error offset from content-relative to raw-token-relative
    .map_err(|offset| prefix_len + offset)?;
    Ok(result.into())
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
    match unescape_string(token.text()) {
        Ok(s) => Some(s),
        Err(offset) => {
            let loc = token.to_source_location();
            diag.push_error_with_span(
                "Cannot parse string literal".into(),
                SourceLocation {
                    source_file: loc.source_file,
                    span: Span::new(
                        loc.span.offset + offset,
                        loc.span.length.saturating_sub(offset),
                    ),
                },
            );
            None
        }
    }
}

#[test]
fn test_unescape_string() {
    assert_eq!(unescape_string(r#""foo_bar""#), Ok("foo_bar".into()));
    assert_eq!(unescape_string(r#""foo\"bar""#), Ok("foo\"bar".into()));
    assert_eq!(unescape_string(r#""foo\\\"bar""#), Ok("foo\\\"bar".into()));
    assert_eq!(unescape_string(r#""fo\na\\r""#), Ok("fo\na\\r".into()));
    assert_eq!(unescape_string(r#""fo\xa""#), Err(3));
    assert_eq!(unescape_string(r#""fooo\""#), Err(5));
    assert_eq!(unescape_string(r#""f\n\n\nf""#), Ok("f\n\n\nf".into()));
    assert_eq!(unescape_string(r#""music\♪xx""#), Err(6));
    assert_eq!(unescape_string(r#""music\"♪\"🎝""#), Ok("music\"♪\"🎝".into()));
    assert_eq!(unescape_string(r#""foo_bar"#), Err(0));
    assert_eq!(unescape_string(r#""foo_bar\"#), Err(0));
    assert_eq!(unescape_string(r#"foo_bar""#), Err(0));
    assert_eq!(unescape_string(r#""d\u{8}a\u{d4}f\u{Ed3}""#), Ok("d\u{8}a\u{d4}f\u{ED3}".into()));
    assert_eq!(unescape_string(r#""xxx\""#), Err(4));
    assert_eq!(unescape_string(r#""xxx\u""#), Err(4));
    assert_eq!(unescape_string(r#""xxx\uxx""#), Err(4));
    assert_eq!(unescape_string(r#""xxx\u{""#), Err(4));
    assert_eq!(unescape_string(r#""xxx\u{22""#), Err(4));
    assert_eq!(unescape_string(r#""xxx\u{qsdf}""#), Err(4));
    assert_eq!(unescape_string(r#""xxx\u{1234567890}""#), Err(4));
}

/// Maps byte offsets in a string assembled from one or more string literal tokens
/// back to precise source locations, accounting for escape sequences.
#[derive(Default)]
pub struct StringLiteralSourceMap {
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

    /// Unescape a string literal token and optionally append the result to
    /// `assembled`, recording the source mapping. Returns the unescaped content,
    /// or reports an error and returns `None`.
    pub fn push(
        &mut self,
        assembled: Option<&mut String>,
        token: &crate::parser::SyntaxToken,
        diag: &mut BuildDiagnostics,
    ) -> Option<SmolStr> {
        let unescaped = unescape_string_reporting(Some(token), diag, token)?;
        let base = assembled.as_ref().map(|a| a.len()).unwrap_or(0);
        if let Some(assembled) = assembled {
            assembled.push_str(&unescaped);
        }
        if unescaped.is_empty() {
            return Some(unescaped);
        }
        let loc = token.to_source_location();
        let token_offset = loc.span.offset;
        let raw = token.text();
        if !raw.contains('\\') {
            // No escapes: the whole content maps 1:1 (skip delimiter)
            self.entries.push(SourceMapEntry {
                assembled_start: base,
                source_offset: token_offset + 1,
                source_file: loc.source_file,
            });
        } else {
            let content = &raw[1..]; // skip opening delimiter
            let content = content
                .strip_suffix('"')
                .or_else(|| content.strip_suffix("\\{"))
                .unwrap_or(content);
            let mut assembled_pos = 0usize;
            let mut source_pos = 1usize; // 1 for opening delimiter
            let mut segment_start_assembled = 0usize;
            let mut segment_start_source = 1usize;
            let _ = walk_escapes(content, |chunk| match chunk {
                EscapeChunk::Plain { len } => {
                    assembled_pos += len;
                    source_pos += len;
                }
                EscapeChunk::Escape { source_len, decoded } => {
                    if assembled_pos > segment_start_assembled {
                        self.entries.push(SourceMapEntry {
                            assembled_start: base + segment_start_assembled,
                            source_offset: token_offset + segment_start_source,
                            source_file: loc.source_file.clone(),
                        });
                    }
                    self.entries.push(SourceMapEntry {
                        assembled_start: base + assembled_pos,
                        source_offset: token_offset + source_pos,
                        source_file: loc.source_file.clone(),
                    });
                    assembled_pos += decoded.len_utf8();
                    source_pos += source_len;
                    segment_start_assembled = assembled_pos;
                    segment_start_source = source_pos;
                }
            });
            if assembled_pos > segment_start_assembled {
                self.entries.push(SourceMapEntry {
                    assembled_start: base + segment_start_assembled,
                    source_offset: token_offset + segment_start_source,
                    source_file: loc.source_file,
                });
            }
        }
        Some(unescaped)
    }

    /// Append a non-literal character (e.g., an interpolation placeholder)
    /// where source and assembled offsets correspond 1:1.
    pub fn push_raw_char(&mut self, assembled: &mut String, ch: char, loc: SourceLocation) {
        let start = assembled.len();
        assembled.push(ch);
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
