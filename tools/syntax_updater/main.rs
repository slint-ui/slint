/*!
Tool to change the syntax or reformat a .60 file

As of know, it just rewrite the exact same as the input, but it can be changed

*/

use sixtyfps_compiler::parser::SyntaxNode;
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
                    dbg!((c, idx, idx2, count));
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

            let (syntax_node, diag) = sixtyfps_compiler::parser::parse(&code);
            if diag.has_error() {
                diag.print(code.into());
            }
            visit_node(syntax_node, &mut file)?
        }
        return file.write_all(source_slice.as_bytes());
    }

    let (syntax_node, mut diag) = sixtyfps_compiler::parser::parse(&source);
    if diag.has_error() {
        diag.current_path = path;
        diag.print(source);
    }

    visit_node(syntax_node, &mut file)
}

fn visit_node(node: SyntaxNode, file: &mut impl Write) -> std::io::Result<()> {
    fold_node(&node, file)?;
    for n in node.children_with_tokens() {
        match n {
            rowan::NodeOrToken::Node(n) => visit_node(n, file)?,
            rowan::NodeOrToken::Token(t) => fold_token(t, file)?,
        };
    }
    Ok(())
}

fn fold_node(_node: &SyntaxNode, _file: &mut impl Write) -> std::io::Result<()> {
    Ok(())
}

fn fold_token(
    node: sixtyfps_compiler::parser::SyntaxToken,
    file: &mut impl Write,
) -> std::io::Result<()> {
    file.write_all(node.text().as_bytes())
}
