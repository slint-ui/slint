//!
//! Tool to change the syntax or reformat a .60 file
//!
//! As of know, it just rewrite the exact same as the input, but it can be changed
//!
//! This is how it can be used:
//!
//! ````shell
//! cargo run --bin syntax_updater -- -i  **/*.60
//! cargo run --bin syntax_updater -- -i  **/*.rs
//! ````

use sixtyfps_compilerlib::parser::{SyntaxKind, SyntaxNode, SyntaxNodeEx};
use std::io::Write;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    #[structopt(name = "path to .60 file(s)", parse(from_os_str))]
    paths: Vec<std::path::PathBuf>,

    /// modify the file inline instead of outputing to stdout
    #[structopt(short, long)]
    inline: bool,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();

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

fn process_file(
    source: String,
    path: std::path::PathBuf,
    mut file: impl Write,
) -> std::io::Result<()> {
    if path.extension().map(|x| x == "rs") == Some(true) {
        let mut source_slice = &source[..];
        let sixtyfps_macro = format!("{}!", "sixtyfps"); // in a variable so it does not appear as is
        'l: while let Some(idx) = source_slice.find(&sixtyfps_macro) {
            // Note: this code ignore string literal and unbalanced comment, but that should be good enough
            let idx2 = if let Some(idx2) =
                source_slice[idx..].find(|c| c == '{' || c == '(' || c == '[')
            {
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

            let (syntax_node, diag) = sixtyfps_compilerlib::parser::parse(code.to_owned());
            let len = syntax_node.text_range().end().into();
            visit_node(syntax_node, &mut file, &mut State::default())?;
            if diag.has_error() {
                file.write_all(&code.as_bytes()[len..])?;
                diag.print();
            }
        }
        return file.write_all(source_slice.as_bytes());
    }

    let (syntax_node, mut diag) = sixtyfps_compilerlib::parser::parse(source.clone());
    let len = syntax_node.text_range().end().into();
    visit_node(syntax_node, &mut file, &mut State::default())?;
    if diag.has_error() {
        file.write_all(&source.as_bytes()[len..])?;
        diag.current_path = path;
        diag.print();
    }
    Ok(())
}

#[derive(Default, Clone)]
struct State {
    property_name: Option<String>,
}

fn visit_node(node: SyntaxNode, file: &mut impl Write, state: &mut State) -> std::io::Result<()> {
    match node.kind() {
        SyntaxKind::PropertyDeclaration => {
            state.property_name = node.child_text(SyntaxKind::DeclaredIdentifier)
        }
        SyntaxKind::Binding => state.property_name = node.child_text(SyntaxKind::Identifier),
        SyntaxKind::SignalDeclaration => {
            state.property_name = node.child_text(SyntaxKind::Identifier)
        }
        _ => (),
    }

    fold_node(&node, file, state)?;
    for n in node.children_with_tokens() {
        match n {
            rowan::NodeOrToken::Node(n) => visit_node(n, file, state)?,
            rowan::NodeOrToken::Token(t) => fold_token(t, file, state)?,
        };
    }
    Ok(())
}

fn fold_node(
    _node: &SyntaxNode,
    _file: &mut impl Write,
    _state: &mut State,
) -> std::io::Result<()> {
    Ok(())
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
