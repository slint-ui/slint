/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
//!
//! Tool to change the syntax or reformat a .60 file
//!
//! As of know, it just rewrite the exact same as the input, but it can be changed
//!
//! This is how it can be used:
//!
//! ````shell
//! cargo run --bin syntax_updater -- --from 0.0.5 -i  **/*.60
//! cargo run --bin syntax_updater -- --from 0.0.5 -i  **/*.rs
//! cargo run --bin syntax_updater -- --from 0.0.5 -i  **/*.md
//! ````

use sixtyfps_compilerlib::diagnostics::BuildDiagnostics;
use sixtyfps_compilerlib::object_tree;
use sixtyfps_compilerlib::parser::{syntax_nodes, NodeOrToken, SyntaxKind, SyntaxNode};
use std::io::Write;
use structopt::StructOpt;

mod from_0_0_5;

#[derive(StructOpt)]
struct Cli {
    #[structopt(name = "path to .60 file(s)", parse(from_os_str))]
    paths: Vec<std::path::PathBuf>,

    /// modify the file inline instead of outputing to stdout
    #[structopt(short, long)]
    inline: bool,

    /// Version to update from
    #[structopt(long, name = "version")]
    from: String,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();

    const SUPPORTED_FROM_VERSION: &str = "0.0.5";

    if args.from != SUPPORTED_FROM_VERSION {
        eprintln!("Only version {0} is supported, use `--from {0}`", SUPPORTED_FROM_VERSION);
        std::process::exit(1);
    }

    for path in args.paths {
        let source = std::fs::read_to_string(&path)?;

        if args.inline {
            let file = std::fs::File::create(&path)?;
            process_file(source, path, file)?
        } else {
            process_file(source, path, std::io::stdout())?
        }
    }
    Ok(())
}

fn process_rust_file(source: String, mut file: impl Write) -> std::io::Result<()> {
    let mut source_slice = &source[..];
    let sixtyfps_macro = format!("{}!", "sixtyfps"); // in a variable so it does not appear as is
    'l: while let Some(idx) = source_slice.find(&sixtyfps_macro) {
        // Note: this code ignore string literal and unbalanced comment, but that should be good enough
        let idx2 =
            if let Some(idx2) = source_slice[idx..].find(|c| c == '{' || c == '(' || c == '[') {
                idx2
            } else {
                break 'l;
            };
        let open = source_slice.as_bytes()[idx + idx2].into();
        let close = match open {
            '{' => '}',
            '(' => ')',
            '[' => ']',
            _ => panic!(),
        };
        file.write_all(source_slice[..=idx + idx2].as_bytes())?;
        source_slice = &source_slice[idx + idx2 + 1..];
        let mut idx = 0;
        let mut count = 1;
        while count > 0 {
            if let Some(idx2) = source_slice[idx..].find(|c| {
                if c == open {
                    count += 1;
                    true
                } else if c == close {
                    count -= 1;
                    true
                } else {
                    false
                }
            }) {
                idx += idx2 + 1;
            } else {
                break 'l;
            }
        }
        let code = &source_slice[..idx - 1];
        source_slice = &source_slice[idx - 1..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = sixtyfps_compilerlib::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        visit_node(syntax_node, &mut file, &mut State::default())?;
        if diag.has_error() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    return file.write_all(source_slice.as_bytes());
}

fn process_markdown_file(source: String, mut file: impl Write) -> std::io::Result<()> {
    let mut source_slice = &source[..];
    const CODE_FENCE_START: &'static str = "```60\n";
    const CODE_FENCE_END: &'static str = "```\n";
    'l: while let Some(code_start) =
        source_slice.find(CODE_FENCE_START).map(|idx| idx + CODE_FENCE_START.len())
    {
        let code_end = if let Some(code_end) = source_slice[code_start..].find(CODE_FENCE_END) {
            code_end
        } else {
            break 'l;
        };
        file.write_all(source_slice[..=code_start - 1].as_bytes())?;
        source_slice = &source_slice[code_start..];
        let code = &source_slice[..code_end];
        source_slice = &source_slice[code_end..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = sixtyfps_compilerlib::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        visit_node(syntax_node, &mut file, &mut State::default())?;
        if diag.has_error() {
            file.write_all(&code.as_bytes()[len..])?;
            diag.print();
        }
    }
    return file.write_all(source_slice.as_bytes());
}

fn process_file(
    source: String,
    path: std::path::PathBuf,
    mut file: impl Write,
) -> std::io::Result<()> {
    match path.extension() {
        Some(ext) if ext == "rs" => return process_rust_file(source, file),
        Some(ext) if ext == "md" => return process_markdown_file(source, file),
        _ => {}
    }

    let mut diag = BuildDiagnostics::default();
    let syntax_node = sixtyfps_compilerlib::parser::parse(source.clone(), Some(&path), &mut diag);
    let len = syntax_node.node.text_range().end().into();
    visit_node(syntax_node, &mut file, &mut State::default())?;
    if diag.has_error() {
        file.write_all(&source.as_bytes()[len..])?;
        diag.print();
    }
    Ok(())
}

#[derive(Default, Clone)]
struct State {
    /// Current element name in scope
    element_name: Option<String>,
    /// When visiting a binding, this is the name of the current property
    property_name: Option<String>,
}

fn visit_node(node: SyntaxNode, file: &mut impl Write, state: &mut State) -> std::io::Result<()> {
    let mut state = state.clone();
    match node.kind() {
        SyntaxKind::PropertyDeclaration => {
            state.property_name = node.child_text(SyntaxKind::DeclaredIdentifier)
        }
        SyntaxKind::Binding => state.property_name = node.child_text(SyntaxKind::Identifier),
        SyntaxKind::CallbackDeclaration => {
            state.property_name = node.child_text(SyntaxKind::Identifier)
        }
        SyntaxKind::Element => {
            let element_node = syntax_nodes::Element(node.clone());
            state.element_name = element_node
                .QualifiedName()
                .map(|qn| object_tree::QualifiedTypeName::from_node(qn).to_string());
        }
        _ => (),
    }

    if fold_node(&node, file, &mut state)? {
        return Ok(());
    }
    for n in node.children_with_tokens() {
        match n {
            NodeOrToken::Node(n) => visit_node(n, file, &mut state)?,
            NodeOrToken::Token(t) => fold_token(t, file, &mut state)?,
        };
    }
    Ok(())
}

/// return false if one need to continue folding the children
fn fold_node(node: &SyntaxNode, file: &mut impl Write, state: &mut State) -> std::io::Result<bool> {
    from_0_0_5::fold_node(node, file, state)
}

fn fold_token(
    node: sixtyfps_compilerlib::parser::SyntaxToken,
    file: &mut impl Write,
    #[allow(unused)] state: &mut State,
) -> std::io::Result<()> {
    /* Example: this adds the "ms" prefix to the number within a "duration" binding
    if state.property_name == Some("duration".into()) && node.kind() == SyntaxKind::NumberLiteral {
        if !node.text().ends_with("s") {
            return write!(file, "{}ms", node.text());
        }
    }*/
    file.write_all(node.text().as_bytes())
}
