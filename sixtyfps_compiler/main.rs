///! FIXME:  remove this
use structopt::StructOpt;

mod parser;

#[derive(StructOpt)]
struct Cli {
    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();
    let source = std::fs::read_to_string(&args.path)?;
    println!("{:#?}", parser::parse(&source));
    Ok(())
}
