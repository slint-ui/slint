#[derive(Default, Debug, Clone, PartialEq, Eq, Hash)]
pub struct CompilerDiagnostic {
    pub message: String,
    pub offset: usize,
}

#[derive(Default, Debug)]
pub struct Diagnostics {
    pub inner: Vec<CompilerDiagnostic>,
}

impl Diagnostics {
    pub fn push_error(&mut self, message: String, offset: usize) {
        self.inner.push(CompilerDiagnostic { message, offset });
    }
}
