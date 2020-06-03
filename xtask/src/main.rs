use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct CMakeOptions {}

#[derive(Debug, StructOpt)]
pub enum Command {
    #[structopt(name = "cmake")]
    CMake(CMakeOptions),
}

#[derive(Debug, StructOpt)]
#[structopt(name = "xtask")]
pub struct ApplicationArguments {
    #[structopt(subcommand)]
    pub command: Command,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let opt = ApplicationArguments::from_args();

    println!("Hello, world!");
    Ok(())
}
