#[derive(Default, Debug, Clone, PartialEq)]
pub struct CompilerDiagnostic {
    pub(super) message: String,
    pub(super) offset: usize,
}

#[derive(Default, Debug)]
pub struct Diagnostics {
    pub(super) inner: Vec<CompilerDiagnostic>,
}

impl Diagnostics {
    pub fn push_error(&mut self, message: String, offset: usize) {
        self.inner.push(CompilerDiagnostic { message, offset });
    }
}
