// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use alloc::boxed::Box;

use super::BreakOpportunity;

pub struct LineBreakIterator<'a>(
    Box<dyn Iterator<Item = (usize, unicode_linebreak::BreakOpportunity)> + 'a>,
);

impl<'a> LineBreakIterator<'a> {
    pub fn new(text: &'a str) -> Self {
        Self(Box::new(unicode_linebreak::linebreaks(text)))
    }
}

impl<'a> Iterator for LineBreakIterator<'a> {
    type Item = (usize, BreakOpportunity);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(byte_offset, opportunity)| {
            (
                byte_offset,
                match opportunity {
                    unicode_linebreak::BreakOpportunity::Mandatory => BreakOpportunity::Mandatory,
                    unicode_linebreak::BreakOpportunity::Allowed => BreakOpportunity::Allowed,
                },
            )
        })
    }
}
