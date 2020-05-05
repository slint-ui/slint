///! FIXME:  remove this
use structopt::StructOpt;

mod diagnostics;
mod object_tree;
mod parser;

#[derive(StructOpt)]
struct Cli {
    #[structopt(name = "path to .60 file", parse(from_os_str))]
    path: std::path::PathBuf,
}

fn main() -> std::io::Result<()> {
    let args = Cli::from_args();
    let source = std::fs::read_to_string(&args.path)?;
    let (res, mut diag) = parser::parse(&source);
    println!("{:#?}", res);
    println!("{:#?}", object_tree::Document::from_node(res, &mut diag));
    if !diag.inner.is_empty() {
        let mut codemap = codemap::CodeMap::new();
        let file = codemap.add_file(args.path.to_string_lossy().into_owned(), source);
        let file_span = file.span;

        let diags: Vec<_> = diag
            .inner
            .into_iter()
            .map(|diagnostics::CompilerDiagnostic { message, offset }| {
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
