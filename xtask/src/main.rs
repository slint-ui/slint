use std::error::Error;
use structopt::StructOpt;

mod cmake;
mod license_headers_check;

#[derive(Debug, StructOpt)]
pub enum TaskCommand {
    #[structopt(name = "cmake")]
    CMake(cmake::CMakeCommand),
    #[structopt(name = "check_license_headers")]
    CheckLicenseHeaders(license_headers_check::LicenseHeaderCheck),
}

#[derive(Debug, StructOpt)]
#[structopt(name = "xtask")]
pub struct ApplicationArguments {
    #[structopt(subcommand)]
    pub command: TaskCommand,
}

fn main() -> Result<(), Box<dyn Error>> {
    match ApplicationArguments::from_args().command {
        TaskCommand::CMake(cmd) => cmd.build_cmake()?,
        TaskCommand::CheckLicenseHeaders(cmd) => cmd.check_license_headers()?,
    };

    Ok(())
}
