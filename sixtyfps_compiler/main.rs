use sixtyfps_compiler::*;
use structopt::StructOpt;

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
    run_passes(&doc, &mut diag, &mut tr);

    let (mut diag, source) = diag.check_and_exit_on_error(source);

    let l = lower::LoweredComponent::lower(&doc.root_component);
    generator::generate(&l, &mut diag);
    diag.check_and_exit_on_error(source);
    Ok(())
}
