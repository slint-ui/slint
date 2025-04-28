// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use i_slint_compiler::parser::SyntaxToken;

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
#[derive(Default)]
pub struct StringWriter {
    content: String,
}

impl StringWriter {
    pub fn finalize(self) -> String {
        self.content
    }
}

impl TokenWriter for StringWriter {
    fn no_change(&mut self, token: SyntaxToken) -> std::io::Result<()> {
        self.content.push_str(token.text());
        Ok(())
    }

    fn with_new_content(&mut self, _token: SyntaxToken, contents: &str) -> std::io::Result<()> {
        self.content.push_str(contents);
        Ok(())
    }

    fn insert_before(&mut self, token: SyntaxToken, contents: &str) -> std::io::Result<()> {
        self.content.push_str(contents);
        self.content.push_str(token.text());
        Ok(())
    }
}
