use structopt::StructOpt;

#[derive(StructOpt)]
struct Cli {
    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();
    let source = std::fs::read_to_string(&args.path)?;
    let c = match sixtyfps_interpreter::load(source, &args.path) {
        Ok(c) => c,
        Err(diag) => {
            diag.print();
            std::process::exit(-1);
        }
    };

    let window = sixtyfps_rendering_backend_gl::create_gl_window();
    let component = c.create();
    window.run(component.borrow(), &component.window_properties());
    Ok(())
}
