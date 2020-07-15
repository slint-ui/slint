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
    pub source: Option<String>,
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
    pub fn print(self) {
        let mut codemap = codemap::CodeMap::new();
        let file = codemap.add_file(
            self.current_path.to_string_lossy().to_string(),
            self.source.unwrap_or_default(),
        );
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
    /// Print into a string
    pub fn diagnostics_as_string(self) -> String {
        let mut codemap = codemap::CodeMap::new();
        let file = codemap.add_file(
            self.current_path.to_string_lossy().to_string(),
            self.source.unwrap_or_default(),
        );
        let file_span = file.span;

        let diags: Vec<_> = self
            .inner
            .iter()
            .map(|CompilerDiagnostic { message, span }| {
                let s = codemap_diagnostic::SpanLabel {
                    span: file_span.subspan(span.offset as u64, span.offset as u64),
                    style: codemap_diagnostic::SpanStyle::Primary,
                    label: None,
                };
                codemap_diagnostic::Diagnostic {
                    level: codemap_diagnostic::Level::Error,
                    message: message.clone(),
                    code: None,
                    spans: vec![s],
                }
            })
            .collect();

        let mut output = Vec::new();
        {
            let mut emitter = codemap_diagnostic::Emitter::vec(&mut output, Some(&codemap));
            emitter.emit(&diags);
        }

        String::from_utf8(output).expect(&format!(
            "There were errors compiling {} but they did not result in valid utf-8 diagnostics!",
            file.name()
        ))
    }

    #[cfg(feature = "display-diagnostics")]
    pub fn check_and_exit_on_error(self) -> Self {
        if self.has_error() {
            self.print();
            std::process::exit(-1);
        }
        self
    }

    pub fn check_errors(self) -> std::io::Result<Self> {
        if !self.has_error() {
            return Ok(self);
        }
        #[cfg(feature = "display-diagnostics")]
        return Err(std::io::Error::new(std::io::ErrorKind::Other, self.diagnostics_as_string()));
        #[cfg(not(feature = "display-diagnostics"))]
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!(
                "Error compiling {} but diagnostics were disabled in the compiler",
                self.current_path.to_string_lossy()
            ),
        ));
    }

    #[cfg(feature = "proc_macro_span")]
    /// Will convert the diagnostics that only have offsets to the actual span
    pub fn map_offsets_to_span(&mut self, span_map: &[crate::parser::Token]) {
        for d in &mut self.inner {
            if d.span.span.is_none() {
                //let pos =
                //span_map.binary_search_by_key(d.span.offset, |x| x.0).unwrap_or_else(|x| x);
                //d.span.span = span_map.get(pos).as_ref().map(|x| x.1);
                let mut offset = 0;
                d.span.span = span_map.iter().find_map(|t| {
                    if d.span.offset <= offset {
                        t.span
                    } else {
                        offset += t.text.len();
                        None
                    }
                });
            }
        }
    }
}

#[cfg(feature = "proc_macro_span")]
use quote::quote;

#[cfg(feature = "proc_macro_span")]
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
