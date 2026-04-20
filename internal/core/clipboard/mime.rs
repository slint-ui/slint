// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use core::fmt::{Debug, Display};
use std::borrow::Cow;

#[derive(PartialEq, Eq, Hash, Clone)]
pub struct Mime {
    inner: Cow<'static, str>,
}

impl Display for Mime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl Debug for Mime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Debug::fmt(&self.inner, f)
    }
}

impl Mime {
    pub const TEXT_PLAIN: Mime = Mime::from_static("text/plain");
    pub const TEXT_PLAIN_UTF_8: Mime = Mime::from_static("text/plain;charset=utf-8");

    const fn from_static(str: &'static str) -> Self {
        Mime { inner: Cow::Borrowed(str) }
    }

    pub fn is_plaintext(&self) -> bool {
        Self::plaintext().contains(self)
    }

    pub fn plaintext() -> &'static [Self] {
        &PLAINTEXT_MIME_TYPES
    }
}

static PLAINTEXT_MIME_TYPES: [Mime; 2] = [Mime::TEXT_PLAIN_UTF_8, Mime::TEXT_PLAIN];
