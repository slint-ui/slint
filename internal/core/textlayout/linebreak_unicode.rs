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

impl LineBreakIterator<'_> {
    pub fn new(text: &str) -> Self {
        // unicode-linebreaks emits a mandatory break at the end of the text, per UAX #14 rule LB3.
        // That break is a separator's own when the text ends with one, and a line follows it.
        let ends_on_break = text.chars().next_back().is_some_and(|c| {
            use unicode_linebreak::BreakClass;
            matches!(
                unicode_linebreak::break_property(c as u32),
                BreakClass::Mandatory
                    | BreakClass::CarriageReturn
                    | BreakClass::LineFeed
                    | BreakClass::NextLine
            )
        });
        let text_len = text.len();
        let iterator = unicode_linebreak::linebreaks(text)
            .filter(move |(offset, _)| ends_on_break || *offset != text_len);

        Self { breaks: iterator.collect(), pos: 0, phantom: Default::default() }
    }
}

impl Iterator for LineBreakIterator<'_> {
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
