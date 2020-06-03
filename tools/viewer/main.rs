use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();
    let source = std::fs::read_to_string(&args.path)?;
    let c = match interpreter::load(source.as_str(), &args.path) {
        Ok(c) => c,
        Err(diag) => {
            diag.print(source);
            std::process::exit(-1);
        }
    };

    let component = c.create();
    gl::sixtyfps_runtime_run_component_with_gl_renderer(component.leak());
    Ok(())
}
