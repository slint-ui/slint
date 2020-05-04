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
    let res = parser::parse(&source);
    println!("{:#?}", res.0);
    if !res.1.is_empty() {
        let mut codemap = codemap::CodeMap::new();
        let file = codemap.add_file(args.path.to_string_lossy().into_owned(), source);
        let file_span = file.span;

        let diags: Vec<_> = res
            .1
            .into_iter()
            .map(|parser::ParseError(message, offset)| {
                let s = codemap_diagnostic::SpanLabel {
                    span: file_span.subspan(offset as u64, offset as u64),
                    style: codemap_diagnostic::SpanStyle::Primary,
                    label: None,
                };
                codemap_diagnostic::Diagnostic {
                    level: codemap_diagnostic::Level::Error,
                    message,
                    code: None,
                    spans: vec![s],
                }
            })
            .collect();

        let mut emitter = codemap_diagnostic::Emitter::stderr(
            codemap_diagnostic::ColorConfig::Always,
            Some(&codemap),
        );
        emitter.emit(&diags);
    }
    Ok(())
}
