// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Phase 3 of the query-based formatter: realize a [`FormatPlan`] through a
//! [`TokenWriter`].
//!
//! Deliberately almost nothing happens here — the plan already contains every
//! decision. This module only turns [`Whitespace`] values into strings and
//! honors the writer protocol: every token of the original tree, trivia
//! included, passes through the writer exactly once.

use super::atoms::{FormatPlan, Instruction, Whitespace};
use super::engine::Linearization;
use super::writer::TokenWriter;

const INDENT: &str = "    ";

pub fn render(
    plan: &FormatPlan,
    linearization: &Linearization,
    writer: &mut impl TokenWriter,
) -> std::io::Result<()> {
    for instruction in &plan.instructions {
        match *instruction {
            Instruction::KeepGap { slot } => {
                for trivia in &linearization.slots[slot].gap_before {
                    writer.no_change(trivia.clone())?;
                }
            }
            Instruction::ReplaceGap { slot, whitespace } => {
                let text = whitespace_text(whitespace);
                // Only comment-free gaps are replaced (the resolver keeps
                // gaps with comments), and those have at most one whitespace
                // token.
                match linearization.slots[slot].single_whitespace_token() {
                    Some(token) => writer.with_new_content(token.clone(), &text)?,
                    None if !text.is_empty() => writer.insert_content(&text)?,
                    None => {}
                }
            }
            Instruction::EmitToken { slot } => {
                writer.no_change(linearization.slots[slot].token.clone())?;
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
