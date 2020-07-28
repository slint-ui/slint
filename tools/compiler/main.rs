use sixtyfps_compilerlib::*;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    /// Set output format
    #[structopt(short = "f", long = "format", default_value = "cpp")]
    format: generator::OutputFormat,

    #[structopt(short = "I", name = "include path for other .60 files", number_of_values = 1)]
    include_paths: Vec<std::path::PathBuf>,

    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();
    let (syntax_node, diag) = parser::parse_file(&args.path)?;
    //println!("{:#?}", syntax_node);
    let compiler_config =
        CompilerConfiguration { include_paths: &args.include_paths, ..Default::default() };
    let (root_component, diag) = compile_syntax_node(syntax_node, diag, &compiler_config);

    let mut diag = diag.check_and_exit_on_error();

    generator::generate(args.format, &mut std::io::stdout(), &root_component, &mut diag)?;
    diag.check_and_exit_on_error();
    Ok(())
}
