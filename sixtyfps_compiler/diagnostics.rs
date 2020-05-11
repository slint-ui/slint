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

#[derive(Default, Debug)]
pub struct Diagnostics {
    pub inner: Vec<CompilerDiagnostic>,
}

impl Diagnostics {
    pub fn push_error(&mut self, message: String, span: Span) {
        self.inner.push(CompilerDiagnostic { message, span });
    }

    pub fn has_error(&self) -> bool {
        !self.inner.is_empty()
    }
}

impl IntoIterator for Diagnostics {
    type Item = CompilerDiagnostic;
    type IntoIter = <Vec<CompilerDiagnostic> as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}
