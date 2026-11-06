// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore poedit nctx nplurals xgettext

use clap::Parser;
use i_slint_compiler::diagnostics::{BuildDiagnostics, Spanned};
use i_slint_compiler::generator::OutputFormat;
use i_slint_compiler::parser::{SyntaxKind, SyntaxNode, syntax_nodes};
use i_slint_compiler::project_file;
use i_slint_compiler::source_path::SourcePath;
use i_slint_compiler::typeloader::TypeLoader;
use rspolib::Save;
use smol_str::SmolStr;

type Messages = rspolib::POFile;

#[derive(clap::Parser, Default)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Specify .slint files, project directories, or slint-project.json files to extract from.
    /// A project directory or slint-project.json file also selects every file that its main file imports.
    #[arg(name = "path to .slint file(s) or project", action, required = true)]
    paths: Vec<std::path::PathBuf>,

    #[arg(long = "default-domain", short = 'd')]
    domain: Option<String>,

    #[arg(
        name = "file",
        short = 'o',
        help = "Write output to specified file (instead of messages.po)."
    )]
    output: Option<std::path::PathBuf>,

    //#[arg(long = "omit-header", help = r#"Don’t write header with ‘msgid ""’ entry"#)]
    //omit_header: bool,
    //
    //#[arg(long = "copyright-holder", help = "Set the copyright holder in the output")]
    //copyright_holder: Option<String>,
    //
    #[arg(long = "package-name", help = "Set the package name in the header of the output")]
    package_name: Option<String>,

    #[arg(long = "package-version", help = "Set the package version in the header of the output")]
    package_version: Option<String>,
    //
    // #[arg(
    //     long = "msgid-bugs-address",
    //     help = "Set the reporting address for msgid bugs. This is the email address or URL to which the translators shall report bugs in the untranslated strings"
    // )]
    // msgid_bugs_address: Option<String>,
    #[arg(long = "join-existing", short = 'j')]
    /// Join messages with existing file
    join_existing: bool,

    #[arg(
        long = "no-default-translation-context",
        help = "Do not set the default context (component name) if none is specified. The Slint compiler need to be configured similarly"
    )]
    no_default_translation_context: bool,
}

fn main() -> std::io::Result<()> {
    let args = Cli::parse();

    let output = args
        .output
        .clone()
        .unwrap_or_else(|| format!("{}.po", args.domain.as_deref().unwrap_or("messages")).into());

    let mut messages = if args.join_existing {
        rspolib::pofile(&*output).map_err(std::io::Error::other)?
    } else {
        let package = args.package_name.as_ref().map(|x| x.as_ref()).unwrap_or("PACKAGE");
        let version = args.package_version.as_ref().map(|x| x.as_ref()).unwrap_or("VERSION");

        let mut file = rspolib::POFile::new(Default::default());
        file.metadata.insert("Project-Id-Version".into(), format!("{package} {version}"));
        file.metadata.insert(
            "POT-Creation-Date".into(),
            chrono::Utc::now().format("%Y-%m-%d %H:%M%z").to_string(),
        );
        file.metadata.insert("PO-Revision-Date".into(), "YEAR-MO-DA HO:MI+ZONE".into());
        file.metadata.insert("Last-Translator".into(), "FULL NAME <EMAIL@ADDRESS>".into());
        file.metadata.insert("Language-Team".into(), "LANGUAGE <LL@li.org>".into());
        file.metadata.insert("MIME-Version".into(), "1.0".into());
        file.metadata.insert("Content-Type".into(), "text/plain; charset=UTF-8".into());
        file.metadata.insert("Content-Transfer-Encoding".into(), "8bit".into());
        file.metadata.insert("Language".into(), String::new());
        file.metadata.insert("Plural-Forms".into(), "nplurals=1; plural=0;".into());

        file
    };

    let mut diag = BuildDiagnostics::default();
    for path in &args.paths {
        let path_diag = if project_file::project_root(path).is_dir() {
            process_project(path, &mut messages, &args)
        } else {
            process_file(path, &mut messages, &args)
        };
        for diagnostic in path_diag {
            diag.push_compiler_error(diagnostic);
        }
    }
    diag.print_warnings_and_exit_on_error();

    messages.save(&output.to_string_lossy());
    Ok(())
}

fn process_file(path: &std::path::Path, messages: &mut Messages, args: &Cli) -> BuildDiagnostics {
    let mut diag = BuildDiagnostics::default();
    if let Some(syntax_node) = i_slint_compiler::parser::parse_file(path, &mut diag) {
        let file = syntax_node.source_file.path().to_string();
        visit_node(syntax_node, messages, None, &file, args);
    }
    diag
}

/// Extracts the messages of the main file of the project at `path`,
/// a directory or a project file, and of every file that it imports.
fn process_project(
    path: &std::path::Path,
    messages: &mut Messages,
    args: &Cli,
) -> BuildDiagnostics {
    /// `path` relative to the absolute `base`, with `/` separators as gettext tools expect.
    /// Returns `None` if they don't share a root, such as two drives on Windows.
    fn relative_path(path: &std::path::Path, base: &std::path::Path) -> Option<String> {
        let path = base.join(path);
        let shares_root = path.components().next() == base.components().next();
        let mut path_components = path.components().peekable();
        let mut base_components = base.components().peekable();
        while let (Some(a), Some(b)) = (path_components.peek(), base_components.peek())
            && a == b
        {
            path_components.next();
            base_components.next();
        }
        shares_root.then(|| {
            base_components
                .map(|_| "..".into())
                .chain(path_components.map(|component| component.as_os_str().to_string_lossy()))
                .collect::<Vec<_>>()
                .join("/")
        })
    }

    let mut diag = BuildDiagnostics::default();
    let (input, _) = poll_ready(project_file::load_input(
        &SourcePath::File(path.into()),
        None,
        None,
        None,
        &mut diag,
    ));
    let Some(input) = input else { return diag };
    let config = input
        .project_file
        .expect("load_input returns the project file of a project directory")
        .into_compiler_configuration(OutputFormat::Llr);
    let mut loader = TypeLoader::new(config, &mut diag);
    poll_ready(loader.load_file(&input.source, input.source_code, false, &mut diag));

    let mut documents =
        loader.all_file_documents().filter(|(path, _)| !path.is_builtin()).collect::<Vec<_>>();
    documents.sort_by_key(|(path, _)| *path);
    let current_dir = std::env::current_dir().ok();
    for (path, document) in documents {
        let file = current_dir
            .as_deref()
            .zip(path.as_native_path())
            .and_then(|(base, path)| relative_path(path, base))
            .unwrap_or_else(|| path.to_string());
        visit_node(document.clone().into(), messages, None, &file, args);
    }
    diag
}

/// Runs a compiler future that has no file loader, so it completes on the first poll.
fn poll_ready<F: std::future::Future>(future: F) -> F::Output {
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    let std::task::Poll::Ready(result) = std::pin::pin!(future).poll(&mut context) else {
        unreachable!("the compiler returned Pending without a file loader")
    };
    result
}

fn visit_node(
    node: SyntaxNode,
    results: &mut Messages,
    current_context: Option<SmolStr>,
    file: &str,
    args: &Cli,
) {
    for n in node.children() {
        if n.kind() == SyntaxKind::AtTr
            && let Some(msgid) = n
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

            let update = |msg: &mut rspolib::POEntry| {
                let span = node.span();
                if span.is_valid() {
                    let (line, _) = node
                        .source_file
                        .line_column(span.offset, i_slint_compiler::diagnostics::ByteFormat::Utf8);
                    if line > 0 {
                        let path = file.to_string();
                        let lineno = line.to_string();
                        if !msg.occurrences.iter().any(|(p, l)| p == &path && l == &lineno) {
                            msg.occurrences.push((path, lineno));
                        }
                    }
                }

                if msg.comment.as_deref().unwrap_or("").is_empty()
                    && let Some(c) = tr
                        .child_token(SyntaxKind::StringLiteral)
                        .and_then(get_comments_before_line)
                        .or_else(|| tr.first_token().and_then(get_comments_before_line))
                {
                    msg.comment = Some(c);
                }
            };

            // Try to find an existing entry in the PO file
            let existing = results.entries.iter_mut().find(|e| {
                e.msgid == msgid
                    && e.msgctxt.as_deref() == msgctxt.as_deref()
                    && if let Some(ref p) = plural {
                        e.msgid_plural.as_deref() == Some(p.as_str())
                    } else {
                        e.msgid_plural.is_none()
                    }
            });

            if let Some(x) = existing {
                update(x);
            } else {
                let mut msg = rspolib::POEntry { msgid: msgid.into(), ..Default::default() };
                if let Some(ref p) = plural {
                    msg.msgid_plural = Some(p.to_string());
                    // Workaround for #4238 : poedit doesn't add the plural by default.
                    msg.msgstr_plural = vec![String::new(), String::new()];
                } else {
                    msg.msgstr = Some(String::new());
                }
                if let Some(msgctxt) = msgctxt {
                    msg.msgctxt = Some(msgctxt.into());
                }
                update(&mut msg);
                // Append or replace
                if let Some(pos) = results
                    .entries
                    .iter()
                    .position(|e| e.msgid == msg.msgid && e.msgctxt == msg.msgctxt)
                {
                    results.entries[pos] = msg;
                } else {
                    results.entries.push(msg);
                }
            }
        }
        let current_context = if !args.no_default_translation_context {
            syntax_nodes::Component::new(n.clone())
                .and_then(|x| {
                    x.DeclaredIdentifier()
                        .child_text(SyntaxKind::Identifier)
                        .map(|t| i_slint_compiler::parser::normalize_identifier(&t))
                })
                .or_else(|| current_context.clone())
        } else {
            None
        };
        visit_node(n, results, current_context, file, args);
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
    use itertools::Itertools;

    #[derive(PartialEq, Debug)]
    pub struct M<'a> {
        pub msgid: &'a str,
        pub msgctx: &'a str,
        pub plural: &'a str,
        pub comments: &'a str,
        pub locations: String,
    }

    impl M<'static> {
        pub fn new(
            msgid: &'static str,
            plural: &'static str,
            msgctx: &'static str,
            comments: &'static str,
            locations: &'static [usize],
        ) -> Self {
            let locations = locations.iter().map(|l| format!("test.slint:{l}",)).join(" ");
            Self { msgid, msgctx, plural, comments, locations }
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

        // comment 5
        d: @tr("dup1");
        d: @tr("ctx" => "dup1");
        d: @tr("dup1");
        // comment 6
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
        M::new("Message 1", "", "Foo", "comment 1", &[3]),
        M::new("Message 2", "", "ctx", "comment 2", &[7]),
        M::new("Message 3", "Messages 3", "Foo", "", &[10]),
        M::new("Message 4", "Messages 4", "ctx4", "comment 4", &[13]),
        M::new("rec1 {}", "", "Foo", "recursive", &[16]),
        M::new("rec2", "", "Foo", "recursive", &[16]),
        M::new("r\nw", "", "rw\nctx", "", &[18]),
        M::new("multi-line\nsecond line", "", "Foo", "multi line", &[21]),
        M::new("dup1", "", "Foo", "comment 5", &[27, 29]),
        M::new("dup1", "", "ctx", "comment 6", &[28, 31]),
        M::new("x", "", "Foo", "macro and string on different line", &[35]),
        M::new("Global", "", "Xx-x", "", &[40]),
    ];

    let mut diag = BuildDiagnostics::default();
    let syntax_node = i_slint_compiler::parser::parse(
        source.into(),
        Some(i_slint_compiler::source_path::SourcePath::new("test.slint")),
        &mut diag,
    );

    let mut messages = rspolib::POFile::new(Default::default());
    visit_node(syntax_node, &mut messages, None, "test.slint", &Cli::default());

    for (a, b) in r.iter().zip(messages.entries.iter()) {
        let locations = b.occurrences.iter().map(|(p, l)| format!("{p}:{l}")).join(" ");
        assert_eq!(
            *a,
            M {
                msgid: b.msgid.as_str(),
                msgctx: b.msgctxt.as_deref().unwrap_or_default(),
                plural: b.msgid_plural.as_deref().unwrap_or_default(),
                comments: b.comment.as_deref().unwrap_or_default(),
                locations,
            }
        );
    }
    assert_eq!(r.len(), messages.entries.len());
}
