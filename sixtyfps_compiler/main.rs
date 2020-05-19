///! FIXME:  remove this
use structopt::StructOpt;

#[cfg(feature = "proc_macro_span")]
extern crate proc_macro;

mod diagnostics;
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
    let tr = typeregister::TypeRegister::builtin();
    let tree = object_tree::Document::from_node(syntax_node, &mut diag, &tr);
    //println!("{:#?}", tree);
    if !diag.inner.is_empty() {
        diag.print(source);
        std::process::exit(-1);
    }

    let l = lower::LoweredComponent::lower(&*tree.root_component);
    generator::generate(&l);
    Ok(())
}
