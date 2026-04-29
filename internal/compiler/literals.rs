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
/// calling `callback` for each chunk. Returns `None` on malformed escapes.
fn walk_escapes(content: &str, mut callback: impl FnMut(EscapeChunk)) -> Option<()> {
    let mut pos = 0;
    while pos < content.len() {
        if content.as_bytes()[pos] == b'\\' {
            if pos + 1 >= content.len() {
                return None;
            }
            match content.as_bytes()[pos + 1] {
                b'"' => {
                    callback(EscapeChunk::Escape { source_len: 2, decoded: '"' });
                    pos += 2;
                }
                b'\\' => {
                    callback(EscapeChunk::Escape { source_len: 2, decoded: '\\' });
                    pos += 2;
                }
                b'n' => {
                    callback(EscapeChunk::Escape { source_len: 2, decoded: '\n' });
                    pos += 2;
                }
                b'u' => {
                    let brace_start = pos + 2;
                    if content.as_bytes().get(brace_start)? != &b'{' {
                        return None;
                    }
                    let brace_end = content[brace_start..].find('}')? + brace_start;
                    let hex = &content[brace_start + 1..brace_end];
                    let x = u32::from_str_radix(hex, 16).ok()?;
                    let ch = std::char::from_u32(x)?;
                    let source_len = brace_end + 1 - pos;
                    callback(EscapeChunk::Escape { source_len, decoded: ch });
                    pos = brace_end + 1;
                }
                _ => return None,
            }
        } else {
            let ch_len = content[pos..].chars().next().map(|c| c.len_utf8()).unwrap_or(1);
            callback(EscapeChunk::Plain { len: ch_len });
            pos += ch_len;
        }
    }
    Some(())
}

pub fn unescape_string(string: &str) -> Option<SmolStr> {
    if string.contains('\n') {
        // FIXME: new line in string literal not yet supported
        return None;
    }
    let string = string.strip_prefix('"').or_else(|| string.strip_prefix('}'))?;
    let string = string.strip_suffix('"').or_else(|| string.strip_suffix("\\{"))?;
    if !string.contains('\\') {
        return Some(string.into());
    }
    let mut result = String::with_capacity(string.len());
    let mut pos = 0;
    walk_escapes(string, |chunk| match chunk {
        EscapeChunk::Plain { len } => {
            result += &string[pos..pos + len];
            pos += len;
        }
        EscapeChunk::Escape { source_len, decoded } => {
            result.push(decoded);
            pos += source_len;
        }
    })?;
    Some(result.into())
}

#[test]
fn test_unescape_string() {
    assert_eq!(unescape_string(r#""foo_bar""#), Some("foo_bar".into()));
    assert_eq!(unescape_string(r#""foo\"bar""#), Some("foo\"bar".into()));
    assert_eq!(unescape_string(r#""foo\\\"bar""#), Some("foo\\\"bar".into()));
    assert_eq!(unescape_string(r#""fo\na\\r""#), Some("fo\na\\r".into()));
    assert_eq!(unescape_string(r#""fo\xa""#), None);
    assert_eq!(unescape_string(r#""fooo\""#), None);
    assert_eq!(unescape_string(r#""f\n\n\nf""#), Some("f\n\n\nf".into()));
    assert_eq!(unescape_string(r#""music\♪xx""#), None);
    assert_eq!(unescape_string(r#""music\"♪\"🎝""#), Some("music\"♪\"🎝".into()));
    assert_eq!(unescape_string(r#""foo_bar"#), None);
    assert_eq!(unescape_string(r#""foo_bar\"#), None);
    assert_eq!(unescape_string(r#"foo_bar""#), None);
    assert_eq!(unescape_string(r#""d\u{8}a\u{d4}f\u{Ed3}""#), Some("d\u{8}a\u{d4}f\u{ED3}".into()));
    assert_eq!(unescape_string(r#""xxx\""#), None);
    assert_eq!(unescape_string(r#""xxx\u""#), None);
    assert_eq!(unescape_string(r#""xxx\uxx""#), None);
    assert_eq!(unescape_string(r#""xxx\u{""#), None);
    assert_eq!(unescape_string(r#""xxx\u{22""#), None);
    assert_eq!(unescape_string(r#""xxx\u{qsdf}""#), None);
    assert_eq!(unescape_string(r#""xxx\u{1234567890}""#), None);
}

/// Given the raw text of a string literal token (e.g. `"hello\nworld"`), maps an
/// offset within the unescaped content back to the corresponding byte offset within
/// the raw token text.
fn source_offset_in_string_literal(raw_token: &str, unescaped_offset: usize) -> usize {
    // Skip the opening delimiter (" or })
    let content = &raw_token[1..];
    let mut source_pos = 0;
    let mut unescaped_pos = 0;

    walk_escapes(content, |chunk| {
        if unescaped_pos >= unescaped_offset {
            return;
        }
        let (source_bytes, unescaped_bytes) = match chunk {
            EscapeChunk::Plain { len } => (len, len),
            EscapeChunk::Escape { source_len, decoded } => (source_len, decoded.len_utf8()),
        };
        if unescaped_pos + unescaped_bytes > unescaped_offset {
            if let EscapeChunk::Plain { .. } = chunk {
                source_pos += unescaped_offset - unescaped_pos;
                unescaped_pos = unescaped_offset;
            }
            // Escape: can't split, point at the start of the escape sequence
        } else {
            source_pos += source_bytes;
            unescaped_pos += unescaped_bytes;
        }
    });
    // +1 for the opening delimiter we skipped
    1 + source_pos
}

#[test]
fn test_source_offset_in_string_literal() {
    // No escapes: offset maps 1:1 (plus 1 for the opening quote)
    assert_eq!(source_offset_in_string_literal(r#""hello""#, 0), 1);
    assert_eq!(source_offset_in_string_literal(r#""hello""#, 3), 4);

    // \n is 2 source bytes but 1 unescaped byte
    // "fo\nbar" -> unescaped "fo\nbar" (7 bytes), source content is fo\nbar (7 bytes)
    assert_eq!(source_offset_in_string_literal(r#""fo\nbar""#, 0), 1); // 'f'
    assert_eq!(source_offset_in_string_literal(r#""fo\nbar""#, 2), 3); // '\' of \n
    assert_eq!(source_offset_in_string_literal(r#""fo\nbar""#, 3), 5); // 'b' after \n

    // \" is 2 source bytes but 1 unescaped byte
    assert_eq!(source_offset_in_string_literal(r#""a\"b""#, 1), 2); // '\"'
    assert_eq!(source_offset_in_string_literal(r#""a\"b""#, 2), 4); // 'b'

    // \u{41} is 6 source bytes but 1 unescaped byte ('A')
    assert_eq!(source_offset_in_string_literal(r#""x\u{41}y""#, 1), 2); // '\u{41}'
    assert_eq!(source_offset_in_string_literal(r#""x\u{41}y""#, 2), 8); // 'y'

    // Multi-byte unescaped char: \u{1F600} is 10 source bytes but 4 unescaped bytes
    assert_eq!(source_offset_in_string_literal(r#""a\u{1F600}b""#, 1), 2); // '\u{1F600}'
    assert_eq!(source_offset_in_string_literal(r#""a\u{1F600}b""#, 5), 11); // 'b'

    // String template fragment starting with }
    assert_eq!(source_offset_in_string_literal("}bar\"", 0), 1);
    assert_eq!(source_offset_in_string_literal("}bar\"", 2), 3);
}

/// Maps byte offsets in a string assembled from one or more string literal tokens
/// back to precise source locations, accounting for escape sequences.
pub struct StringLiteralSourceMap {
    entries: Vec<SourceMapEntry>,
}

struct SourceMapEntry {
    /// Byte range in the assembled (unescaped) string
    range: std::ops::Range<usize>,
    /// Raw text of the source token (with quotes/delimiters)
    raw_token: SmolStr,
    /// Source location of the full token in the .slint source
    loc: SourceLocation,
}

impl StringLiteralSourceMap {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Create a source map for a single string literal token.
    pub fn from_token(token: &dyn Spanned, raw_text: &str, unescaped: &str) -> Self {
        let mut map = Self::new();
        let mut dummy = String::new();
        map.push_literal(&mut dummy, token, raw_text, unescaped);
        map
    }

    /// Create a source map for a `StringLiteral` expression when the raw token text
    /// is not directly available. Recovers it from the source file.
    pub fn from_expression(unescaped: &str, binding: &dyn Spanned) -> Self {
        let raw_and_loc = binding.source_file().and_then(|sf| {
            let src = sf.source()?;
            let span = binding.span();
            // The BindingExpression span is near the string literal but may or may
            // not include the opening quote. Search a small window to find it.
            let search_end = (span.offset + 3).min(src.len());
            let search_begin = span.offset.saturating_sub(3);
            let quote_pos = search_begin + src[search_begin..search_end].rfind('"')?;
            // Find the closing delimiter by trying progressively longer slices
            // until unescape_string succeeds with the expected content.
            // This correctly handles escaped quotes. Only used on error paths.
            let rest = src.get(quote_pos..)?;
            let min_len = 2 + unescaped.len();
            let raw = (min_len..=rest.len()).find_map(|len| {
                let candidate = rest.get(..len)?;
                if !candidate.ends_with('"') && !candidate.ends_with("\\{") {
                    return None;
                }
                let unesc = unescape_string(candidate)?;
                if *unesc == *unescaped { Some(candidate) } else { None }
            })?;
            let loc = SourceLocation {
                source_file: Some(sf.clone()),
                span: Span::new(quote_pos, raw.len()),
            };
            Some((SmolStr::from(raw), loc))
        });
        let mut map = Self::new();
        if let Some((raw_token, loc)) = raw_and_loc {
            map.entries.push(SourceMapEntry { range: 0..unescaped.len(), raw_token, loc });
        }
        map
    }

    /// Append content from a string literal token. The unescaped content is appended to
    /// `assembled`, and the mapping from the raw token is recorded.
    pub fn push_literal(
        &mut self,
        assembled: &mut String,
        token: &dyn Spanned,
        raw_text: &str,
        unescaped: &str,
    ) {
        let start = assembled.len();
        assembled.push_str(unescaped);
        let end = assembled.len();
        if end > start {
            self.entries.push(SourceMapEntry {
                range: start..end,
                raw_token: raw_text.into(),
                loc: token.to_source_location(),
            });
        }
    }

    /// Append a non-literal character (e.g., an interpolation placeholder)
    /// where source and assembled offsets correspond 1:1.
    pub fn push_raw_char(&mut self, assembled: &mut String, ch: char, loc: SourceLocation) {
        let start = assembled.len();
        assembled.push(ch);
        let s: SmolStr = String::from(ch).into();
        self.entries.push(SourceMapEntry { range: start..assembled.len(), raw_token: s, loc });
    }

    /// Resolve a byte range in the assembled string to a precise source location.
    /// The returned span points at the specific position within the string literal.
    pub fn resolve(&self, range: std::ops::Range<usize>) -> Option<SourceLocation> {
        // partition_point returns the first index where entry.range.start > range.start,
        // so idx - 1 is the last entry whose range could contain range.start.
        let idx = self.entries.partition_point(|e| e.range.start <= range.start);
        if idx == 0 {
            return None;
        }
        let entry = &self.entries[idx - 1];
        if !entry.range.contains(&range.start) {
            return None;
        }
        let delta = range.start - entry.range.start;
        let source_offset = source_offset_in_string_literal(&entry.raw_token, delta);
        let err_len = range.len().min(entry.loc.span.length.saturating_sub(source_offset));
        Some(SourceLocation {
            source_file: entry.loc.source_file.clone(),
            span: Span::new(entry.loc.span.offset + source_offset, err_len),
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
