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
        process_file(path, &mut messages)?
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

fn process_file(path: std::path::PathBuf, messages: &mut Messages) -> std::io::Result<()> {
    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse_file(path, &mut diag).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::Other, diag.to_string_vec().join(", "))
    })?;
    visit_node(syntax_node, messages, None);

    Ok(())
}

fn visit_node(node: SyntaxNode, results: &mut Messages, current_context: Option<String>) {
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
                    .and_then(|s| i_slint_compiler::literals::unescape_string(&s))
                    .or_else(|| current_context.clone());
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

                if message.comments.is_none() {
                    message.comments = tr
                        .child_token(SyntaxKind::StringLiteral)
                        .and_then(get_comments_before_line)
                        .or_else(|| tr.first_token().and_then(get_comments_before_line));
                }
            }
        }
        let current_context = syntax_nodes::Component::new(n.clone())
            .and_then(|x| {
                x.DeclaredIdentifier()
                    .child_text(SyntaxKind::Identifier)
                    .map(|t| i_slint_compiler::parser::normalize_identifier(&t))
            })
            .or_else(|| current_context.clone());
        visit_node(n, results, current_context);
    }
}

fn get_comments_before_line(token: i_slint_compiler::parser::SyntaxToken) -> Option<String> {
    let mut token = token.prev_token()?;
    loop {
        if token.kind() == SyntaxKind::Whitespace {
            let mut lines = token.text().lines();
            lines.next();
            if lines.next().is_some() {
                // One \n
                if lines.next().is_some() {
                    return None; // two \n or more
                }
                token = token.prev_token()?;
                if token.kind() == SyntaxKind::Comment && token.text().starts_with("//") {
                    return Some(token.text().trim_start_matches('/').trim().into());
                }
                return None;
            }
        }
        token = token.prev_token()?;
    }
}

#[test]
fn extract_messages() {
    fn make(msg: &str, p: &str, ctx: &str, co: &str, loc: &[usize]) -> Message {
        let opt = |x: &str| (!x.is_empty()).then(|| x.to_owned());
        let locations = loc
            .iter()
            .map(|l| messages::Location { file: "test.slint".to_owned().into(), line: *l })
            .collect();
        Message {
            msgctxt: opt(ctx),
            msgid: msg.into(),
            plural: opt(p),
            locations,
            comments: opt(co),
            index: 0,
        }
    }

    let source = r##"export component Foo {
        // comment 1
        x: @tr("Message 1");
        // comment does not count

        // comment 2
        y: @tr("ctx" => "Message 2");
        // comment  does not count

        z: @tr("Message 3" | "Messages 3" % x);

        // comment 4
        a: @tr("ctx4" => "Message 4" | "Messages 4" % x);

        //recursive
        rec: @tr("rec1 {}", @tr("rec2"));

        nl: @tr("rw\nctx" => "r\nw");

        // comment does not count : xgettext takes the comment next to the string
        xx: @tr(
            //multi line
            "multi-line\nsecond line"
        );

        d: @tr("dup1");
        d: @tr("ctx" => "dup1");
        d: @tr("dup1");
        d: @tr("ctx" => "dup1");

        // two-line-comment
        // macro and string on different line
        x: @tr(
            "x"
        );
    }
    global Xx_x {
        property <string> moo: @tr("Global");
    }
    }"##;

    let r = [
        make("Message 1", "", "Foo", "comment 1", &[3]),
        make("Message 2", "", "ctx", "comment 2", &[7]),
        make("Message 3", "Messages 3", "Foo", "", &[10]),
        make("Message 4", "Messages 4", "ctx4", "comment 4", &[13]),
        make("rec1 {}", "", "Foo", "recursive", &[16]),
        make("rec2", "", "Foo", "recursive", &[16]),
        make("r\nw", "", "rw\nctx", "", &[18]),
        make("multi-line\nsecond line", "", "Foo", "multi line", &[21]),
        make("dup1", "", "Foo", "", &[26, 28]),
        make("dup1", "", "ctx", "", &[27, 29]),
        make("x", "", "Foo", "macro and string on different line", &[33]),
        make("Global", "", "Xx-x", "", &[38]),
    ];

    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse(
        source.into(),
        Some(std::path::Path::new("test.slint")),
        &mut diag,
    );

    let mut messages = Messages::new();
    visit_node(syntax_node, &mut messages, None);

    let mut messages = messages.into_values().collect::<Vec<_>>();
    messages.sort_by_key(|m| m.index);
    let mlen = messages.len();
    for (a, mut b) in r.iter().zip(messages) {
        b.index = 0;
        assert_eq!(*a, b);
    }
    assert_eq!(r.len(), mlen);
}
