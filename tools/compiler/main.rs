use sixtyfps_compilerlib::*;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    /// Set output format
    #[structopt(short = "f", long = "format", default_value = "cpp")]
    format: generator::OutputFormat,

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
    let doc = object_tree::Document::from_node(syntax_node.into(), &mut diag, &tr);
    let compiler_config = CompilerConfiguration::default();
    run_passes(&doc, &mut diag, &compiler_config);

    let (mut diag, source) = diag.check_and_exit_on_error(source);

    generator::generate(args.format, &mut std::io::stdout(), &doc.root_component, &mut diag)?;
    diag.check_and_exit_on_error(source);
    Ok(())
}
