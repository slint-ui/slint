// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use clap::Parser;
use i_slint_compiler::diagnostics::{BuildDiagnostics, Spanned};
use i_slint_compiler::parser::{syntax_nodes, SyntaxKind, SyntaxNode};
use messages::{Message, Messages};

mod generator;
mod messages;

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(name = "path to .slint file(s)", action)]
    paths: Vec<std::path::PathBuf>,

    #[arg(long = "default-domain", short = 'd')]
    domain: Option<String>,

    #[arg(
        name = "file",
        short = 'o',
        help = "Write output to specified file (instead of messages.po)."
    )]
    output: Option<std::path::PathBuf>,

    #[arg(long = "omit-header", help = r#"Don’t write header with ‘msgid ""’ entry"#)]
    omit_header: bool,

    #[arg(long = "copyright-holder", help = "Set the copyright holder in the output")]
    copyright_holder: Option<String>,

    #[arg(long = "package-name", help = "Set the package name in the header of the output")]
    package_name: Option<String>,

    #[arg(long = "package-version", help = "Set the package version in the header of the output")]
    package_version: Option<String>,

    #[arg(
        long = "msgid-bugs-address",
        help = "Set the reporting address for msgid bugs. This is the email address or URL to which the translators shall report bugs in the untranslated strings"
    )]
    msgid_bugs_address: Option<String>,
}

fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    let mut messages = Messages::new();

    for path in args.paths {
        let source = std::fs::read_to_string(&path)?;

        process_file(source, path, &mut messages)?
    }

    let output = args.output.unwrap_or_else(|| {
        format!("{}.po", args.domain.as_ref().map(String::as_str).unwrap_or("messages")).into()
    });

    let output_details = generator::OutputDetails {
        omit_header: args.omit_header,
        copyright_holder: args.copyright_holder,
        package_name: args.package_name,
        package_version: args.package_version,
        bugs_address: args.msgid_bugs_address,
        charset: "UTF-8".into(),
        add_location: generator::AddLocation::Full,
    };

    let mut messages: Vec<_> = messages.values().collect();
    messages.sort_by_key(|m| m.index);

    generator::generate(output, output_details, messages)
}

/// FIXME! this is duplicated with the updater
fn process_rust_file(source: String, _messages: &mut Messages) -> std::io::Result<()> {
    let mut source_slice = &source[..];
    let slint_macro = format!("{}!", "slint"); // in a variable so it does not appear as is
    'l: while let Some(idx) = source_slice.find(&slint_macro) {
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
        todo!("Rust files");
        /*
        let code = &source_slice[..idx - 1];
        source_slice = &source_slice[idx - 1..];

        let mut diag = BuildDiagnostics::default();
        let syntax_node = i_slint_compiler::parser::parse(code.to_owned(), None, &mut diag);
        let len = syntax_node.text_range().end().into();
        visit_node(syntax_node, &mut file)?;
        */
    }
    Ok(())
}

fn process_slint_file(
    source: String,
    path: std::path::PathBuf,
    messages: &mut Messages,
) -> std::io::Result<()> {
    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse(source.clone(), Some(&path), &mut diag);
    visit_node(syntax_node, messages);

    Ok(())
}

fn process_file(
    source: String,
    path: std::path::PathBuf,
    messages: &mut Messages,
) -> std::io::Result<()> {
    match path.extension() {
        Some(ext) if ext == "rs" => process_rust_file(source, messages),
        _ => process_slint_file(source, path, messages),
    }
}

fn visit_node(node: SyntaxNode, results: &mut Messages) {
    for n in node.children() {
        if n.kind() == SyntaxKind::AtTr {
            if let Some(msgid) = n
                .child_text(SyntaxKind::StringLiteral)
                .and_then(|s| i_slint_compiler::literals::unescape_string(&s))
            {
                let tr = syntax_nodes::AtTr::from(n.clone());
                let msgctxt = tr
                    .TrContext()
                    .and_then(|n| n.child_text(SyntaxKind::StringLiteral))
                    .and_then(|s| i_slint_compiler::literals::unescape_string(&s));
                let plural = tr
                    .TrPlural()
                    .and_then(|n| n.child_text(SyntaxKind::StringLiteral))
                    .and_then(|s| i_slint_compiler::literals::unescape_string(&s));
                let key =
                    messages::MessageKey::new(msgid.clone(), msgctxt.clone().unwrap_or_default());
                let index = results.len();
                let message = results.entry(key).or_insert_with(|| Message {
                    msgctxt,
                    msgid,
                    index,
                    plural,
                    ..Default::default()
                });

                let span = node.span();
                if span.is_valid() {
                    let (line, _) = node.source_file.line_column(span.offset);
                    if line > 0 {
                        message.locations.push(messages::Location {
                            file: node.source_file.path().into(),
                            line,
                        });
                    }
                }

                /* TODO
                let mut comments = get_comment_before_line(&self.source_lines, line);
                if comments.is_none() {
                    let ident_line = ident_span.start().line;
                    if ident_line != line {
                        comments = get_comment_before_line(&self.source_lines, ident_line);
                    }
                }
                message.comments = comments;
                */
            }
        }
        visit_node(n, results)
    }
}
