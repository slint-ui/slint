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
    use sixtyfps_compiler::*;
    let args = Cli::from_args();

    for path in args.paths {
        let source = std::fs::read_to_string(&path)?;
        let (syntax_node, mut diag) = parser::parse(&source);
        if diag.has_error() {
            diag.current_path = path;
            diag.print(source);
            continue;
        }

        if args.inline {
            print_document(syntax_node, std::fs::File::create(path)?)?
        } else {
            print_document(syntax_node, std::io::stdout())?
        }
    }
    Ok(())
}

fn print_document(node: SyntaxNode, mut file: impl Write) -> std::io::Result<()> {
    visit_node(node, &mut file)
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
