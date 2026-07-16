// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Phase 3 of the query-based formatter: realize a [`FormatPlan`] through a
//! [`TokenWriter`].
//!
//! Deliberately almost nothing happens here — the plan already contains every
//! decision. This module only turns [`Whitespace`] values into strings and
//! honors the writer protocol: every token of the original tree, trivia
//! included, passes through the writer exactly once.

use super::atoms::{FormatPlan, INDENT, Instruction, Whitespace};
use super::engine::Linearization;
use super::writer::TokenWriter;
use i_slint_compiler::parser::SyntaxToken;

pub fn render(
    plan: &FormatPlan,
    linearization: &Linearization,
    writer: &mut impl TokenWriter,
) -> std::io::Result<()> {
    for instruction in &plan.instructions {
        match instruction {
            &Instruction::KeepGap { slot } => {
                for trivia in &linearization.slots[slot].gap_before {
                    writer.no_change(trivia.clone())?;
                }
            }
            &Instruction::ReplaceGap { slot, whitespace } => {
                // Whole-gap replacement is only produced for comment-free
                // gaps (comment gaps become sub-gap instruction sequences),
                // and those have at most one whitespace token.
                let token = linearization.slots[slot].single_whitespace_token();
                replace_whitespace(token, whitespace, writer)?;
            }
            &Instruction::ReplaceSubGap { slot, trivia_index, whitespace } => {
                let token = trivia_index.map(|index| &linearization.slots[slot].gap_before[index]);
                replace_whitespace(token, whitespace, writer)?;
            }
            &Instruction::EmitComment { slot, trivia_index, column_shift } => {
                let token = &linearization.slots[slot].gap_before[trivia_index];
                match shift_continuation_lines(token.text(), column_shift) {
                    Some(shifted) => writer.with_new_content(token.clone(), &shifted)?,
                    None => writer.no_change(token.clone())?,
                }
            }
            Instruction::EmitLiteral { text } => {
                writer.insert_content(text)?;
            }
            &Instruction::EmitToken { slot } => {
                writer.no_change(linearization.slots[slot].token.clone())?;
            }
            &Instruction::DeleteToken { slot } => {
                // The token still passes the writer once, as empty content.
                writer.with_new_content(linearization.slots[slot].token.clone(), "")?;
            }
        }
    }
    // Only error-truncated trees have trailing trivia; emit it unchanged so
    // no input text is lost.
    for trivia in &linearization.trailing_trivia {
        writer.no_change(trivia.clone())?;
    }
    Ok(())
}

/// Replace `token`'s text with the whitespace — or insert the whitespace
/// where there is no token to replace (and nothing at all when it is empty).
fn replace_whitespace(
    token: Option<&SyntaxToken>,
    whitespace: Whitespace,
    writer: &mut impl TokenWriter,
) -> std::io::Result<()> {
    let text = whitespace_text(whitespace);
    match token {
        Some(token) => writer.with_new_content(token.clone(), &text),
        None if !text.is_empty() => writer.insert_content(&text),
        None => Ok(()),
    }
}

fn whitespace_text(whitespace: Whitespace) -> String {
    match whitespace {
        Whitespace::None => String::new(),
        Whitespace::Space => String::from(" "),
        Whitespace::Newline { blank_line, indentation_level } => {
            let mut text = String::from(if blank_line { "\n\n" } else { "\n" });
            for _ in 0..indentation_level {
                text += INDENT;
            }
            text
        }
    }
}

/// Adjust the leading whitespace of every line after the first by
/// `column_shift` columns (clamped at zero), so a re-indented multiline
/// block comment keeps its internal alignment. Returns `None` when nothing
/// changes (zero shift or single-line comment); whitespace-only lines stay
/// untouched to avoid introducing trailing whitespace. Leading tabs count as
/// one column each and are normalized to spaces.
fn shift_continuation_lines(text: &str, column_shift: i32) -> Option<String> {
    if column_shift == 0 || !text.contains('\n') {
        return None;
    }
    let mut lines = text.split('\n');
    let mut shifted = String::from(lines.next().unwrap_or_default());
    for line in lines {
        shifted.push('\n');
        let content = line.trim_start_matches([' ', '\t']);
        // A `\r\n` line ending leaves the `\r` at the end of this line's
        // slice; a lone `\r` still means the line is blank.
        if content.trim_end_matches('\r').is_empty() {
            shifted.push_str(line);
            continue;
        }
        let leading_columns = (line.len() - content.len()) as i32;
        shifted.push_str(&" ".repeat((leading_columns + column_shift).max(0) as usize));
        shifted.push_str(content);
    }
    Some(shifted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_continuation_lines_preserves_internal_alignment() {
        // Shift right by 4: both continuation lines move together.
        assert_eq!(
            shift_continuation_lines("/* a\n   b\n     c */", 4).unwrap(),
            "/* a\n       b\n         c */"
        );
        // Shift left clamps at column 0 per line.
        assert_eq!(shift_continuation_lines("/* a\n   b\n c */", -2).unwrap(), "/* a\n b\nc */");
        // Whitespace-only lines stay untouched (no trailing whitespace),
        // also with \r\n line endings.
        assert_eq!(shift_continuation_lines("/* a\n\n b */", 4).unwrap(), "/* a\n\n     b */");
        assert_eq!(
            shift_continuation_lines("/* a\r\n\r\n b */", 4).unwrap(),
            "/* a\r\n\r\n     b */"
        );
        // No change needed: single-line comment or zero shift.
        assert!(shift_continuation_lines("// single", 4).is_none());
        assert!(shift_continuation_lines("/* a\n b */", 0).is_none());
    }
}
