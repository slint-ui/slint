// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::parser::SyntaxToken;
use std::io::Write;

/// The idea is that each token need to go through this, either with no changes,
/// or with a new content.
pub trait TokenWriter {
    /// Write token to the writer without any change.
    fn no_change(&mut self, token: SyntaxToken) -> std::io::Result<()>;

    /// Write just contents into the writer (replacing token).
    fn with_new_content(&mut self, token: SyntaxToken, contents: &str) -> std::io::Result<()>;

    /// Write contents and then the token to the writer.
    fn insert_before(&mut self, token: SyntaxToken, contents: &str) -> std::io::Result<()>;
}

/// Just write the token stream to a file
pub struct FileWriter<'a, W> {
    pub file: &'a mut W,
}

impl<W: Write> TokenWriter for FileWriter<'_, W> {
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
