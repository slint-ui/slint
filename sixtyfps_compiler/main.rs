///! FIXME:  remove this
use structopt::StructOpt;

#[cfg(feature = "proc_macro_span")]
extern crate proc_macro;

mod diagnostics;
mod expressions;
mod generator;
mod lower;
mod object_tree;
mod parser;
mod typeregister;

#[derive(StructOpt)]
struct Cli {
    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();
    let source = std::fs::read_to_string(&args.path)?;
    let (syntax_node, mut diag) = parser::parse(&source);
    diag.current_path = args.path;
    //println!("{:#?}", syntax_node);
    let mut tr = typeregister::TypeRegister::builtin();
    let doc = object_tree::Document::from_node(syntax_node, &mut diag, &mut tr);
    expressions::resolve_expressions(&doc, &mut diag, &mut tr);

    //println!("{:#?}", doc);
    if !diag.inner.is_empty() {
        diag.print(source);
        std::process::exit(-1);
    }

    let l = lower::LoweredComponent::lower(&*doc.root_component);
    generator::generate(&l);
    Ok(())
}
