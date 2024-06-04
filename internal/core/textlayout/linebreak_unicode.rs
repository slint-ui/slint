// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::marker::PhantomData;

pub use unicode_linebreak::BreakOpportunity;

use crate::SharedVector;

#[derive(Clone)]
pub struct LineBreakIterator<'a> {
    breaks: SharedVector<(usize, unicode_linebreak::BreakOpportunity)>,
    pos: usize,
    phantom: PhantomData<&'a str>,
}

impl<'a> LineBreakIterator<'a> {
    pub fn new(text: &str) -> Self {
        let iterator = unicode_linebreak::linebreaks(text).filter(|(offset, opportunity)| {
            // unicode-linebreaks emits a mandatory break at the end of the text. We're not interested
            // in that.
            *offset != text.len() || !matches!(opportunity, BreakOpportunity::Mandatory)
        });

        Self { breaks: iterator.collect(), pos: 0, phantom: Default::default() }
    }
}

impl<'a> Iterator for LineBreakIterator<'a> {
    type Item = (usize, unicode_linebreak::BreakOpportunity);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.breaks.len() {
            let i = self.pos;
            self.pos += 1;
            Some(self.breaks[i])
        } else {
            None
        }
    }
}
