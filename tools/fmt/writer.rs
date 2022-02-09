// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)
use i_slint_compiler::parser::SyntaxToken;
use std::io::Write;

/// The idea is that each token need to go through this, either with no changes,
/// or with a new content.
pub(crate) trait TokenWriter {
    /// Write token to the writer without any change.
    fn no_change(&mut self, token: SyntaxToken) -> std::io::Result<()>;

    /// Write just contents into the writer (replacing token).
    fn with_new_content(&mut self, token: SyntaxToken, contents: &str) -> std::io::Result<()>;

    /// Write contents and then the token to the writer.
    fn insert_before(&mut self, token: SyntaxToken, contents: &str) -> std::io::Result<()>;
}

/// Just write the token stream to a file
pub(crate) struct FileWriter<'a, W> {
    pub(super) file: &'a mut W,
}

impl<'a, W: Write> TokenWriter for FileWriter<'a, W> {
    fn no_change(&mut self, token: SyntaxToken) -> std::io::Result<()> {
        self.file.write_all(token.text().as_bytes())
    }

    fn with_new_content(&mut self, _token: SyntaxToken, contents: &str) -> std::io::Result<()> {
        self.file.write_all(contents.as_bytes())
    }

    fn insert_before(&mut self, token: SyntaxToken, contents: &str) -> std::io::Result<()> {
        self.file.write_all(contents.as_bytes())?;
        self.file.write_all(token.text().as_bytes())
    }
}
