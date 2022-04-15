// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::boxed::Box;

pub use unicode_linebreak::BreakOpportunity;

#[derive(derive_more::DerefMut, derive_more::Deref)]
pub struct LineBreakIterator<'a>(
    Box<dyn Iterator<Item = (usize, unicode_linebreak::BreakOpportunity)> + 'a>,
);

impl<'a> LineBreakIterator<'a> {
    pub fn new(text: &'a str) -> Self {
        Self(Box::new(unicode_linebreak::linebreaks(text).filter(|(offset, opportunity)| {
            // unicode-linebreaks emits a mandatory break at the end of the text. We're not interested
            // in that.
            if *offset == text.len() && matches!(opportunity, BreakOpportunity::Mandatory) {
                false
            } else {
                true
            }
        })))
    }
}
