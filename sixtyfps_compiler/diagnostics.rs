#[derive(Debug, Clone, Default)]
pub struct Span {
    pub offset: usize,
    #[cfg(feature = "proc_macro_span")]
    pub span: Option<proc_macro::Span>,
}

impl PartialEq for Span {
    fn eq(&self, other: &Span) -> bool {
        self.offset == other.offset
    }
}

impl Span {
    pub fn new(offset: usize) -> Self {
        Self { offset, ..Default::default() }
    }
}

#[cfg(feature = "proc_macro_span")]
impl From<proc_macro::Span> for Span {
    fn from(span: proc_macro::Span) -> Self {
        Self { span: Some(span), ..Default::default() }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct CompilerDiagnostic {
    pub message: String,
    pub span: Span,
}

#[derive(Default, Debug, Clone)]
pub struct Diagnostics {
    pub inner: Vec<CompilerDiagnostic>,
    pub current_path: std::path::PathBuf,
}

impl IntoIterator for Diagnostics {
    type Item = CompilerDiagnostic;
    type IntoIter = <Vec<CompilerDiagnostic> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

impl Diagnostics {
    pub fn push_error(&mut self, message: String, span: Span) {
        self.inner.push(CompilerDiagnostic { message, span });
    }
    pub fn push_compiler_error(&mut self, error: CompilerDiagnostic) {
        self.inner.push(error);
    }

    pub fn has_error(&self) -> bool {
        !self.inner.is_empty()
    }

    /// Returns the path for a given span
    ///
    /// (currently just return the current path)
    pub fn path(&self, _span: Span) -> &std::path::Path {
        &*self.current_path
    }

    #[cfg(feature = "display-diagnostics")]
    /// Print the diagnostics on the console
    pub fn print(self, source: String) {
        let mut codemap = codemap::CodeMap::new();
        let file = codemap.add_file(self.current_path.to_string_lossy().to_string(), source);
        let file_span = file.span;

        let diags: Vec<_> = self
            .inner
            .into_iter()
            .map(|CompilerDiagnostic { message, span }| {
                let s = codemap_diagnostic::SpanLabel {
                    span: file_span.subspan(span.offset as u64, span.offset as u64),
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

    #[cfg(feature = "display-diagnostics")]
    pub fn check_and_exit_on_error(self, source: String) -> (Self, String) {
        if self.has_error() {
            self.print(source);
            std::process::exit(-1);
        }
        (self, source)
    }
}

#[cfg(feature = "proc_macro_diagnostics")]
use quote::quote;

#[cfg(feature = "proc_macro_diagnostics")]
impl quote::ToTokens for Diagnostics {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let diags: Vec<_> = self
            .clone()
            .into_iter()
            .map(|CompilerDiagnostic { message, span }| {
                if let Some(span) = span.span {
                    quote::quote_spanned!(span.into() => compile_error!{ #message })
                } else {
                    quote!(compile_error! { #message })
                }
            })
            .collect();
        quote!(#(#diags)*).to_tokens(tokens);
    }
}
