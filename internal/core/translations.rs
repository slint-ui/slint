// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use crate::SharedString;
pub use formatter::FormatArgs;

mod formatter {
    use core::fmt::{Display, Formatter, Result};

    pub trait FormatArgs {
        type Output<'a>: Display
        where
            Self: 'a;
        fn from_index<'a>(&'a self, index: usize) -> Option<Self::Output<'a>>;
        fn from_name<'a>(&'a self, _name: &str) -> Option<Self::Output<'a>> {
            None
        }
    }

    impl<T: Display> FormatArgs for [T] {
        type Output<'a> = &'a T where T: 'a;
        fn from_index<'a>(&'a self, index: usize) -> Option<&'a T> {
            self.get(index)
        }
    }

    impl<const N: usize, T: Display> FormatArgs for [T; N] {
        type Output<'a> = &'a T where T: 'a;
        fn from_index<'a>(&'a self, index: usize) -> Option<&'a T> {
            self.get(index)
        }
    }

    pub fn format<'a>(
        format_str: &'a str,
        args: &'a (impl FormatArgs + ?Sized),
    ) -> impl Display + 'a {
        FormatResult { format_str, args }
    }

    struct FormatResult<'a, T: ?Sized> {
        format_str: &'a str,
        args: &'a T,
    }

    impl<'a, T: FormatArgs + ?Sized> Display for FormatResult<'a, T> {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            let mut arg_idx = 0;
            let mut pos = 0;
            while let Some(mut p) = self.format_str[pos..].find(|x| x == '{' || x == '}') {
                if self.format_str.len() - pos < p + 1 {
                    break;
                }
                p += pos;

                // Skip escaped }
                if self.format_str.get(p..=p) == Some("}") {
                    self.format_str[pos..=p].fmt(f)?;
                    if self.format_str.get(p + 1..=p + 1) == Some("}") {
                        pos = p + 2;
                    } else {
                        // FIXME! this is an error, it should be reported  ('}' must be escaped)
                        pos = p + 1;
                    }
                    continue;
                }

                // Skip escaped {
                if self.format_str.get(p + 1..=p + 1) == Some("{") {
                    self.format_str[pos..=p].fmt(f)?;
                    pos = p + 2;
                    continue;
                }

                // Find the argument
                let end = if let Some(end) = self.format_str[p..].find('}') {
                    end + p
                } else {
                    // FIXME! this is an error, it should be reported
                    self.format_str[pos..=p].fmt(f)?;
                    pos = p + 1;
                    continue;
                };
                let argument = self.format_str[p + 1..end].trim();
                let pa = if p == end - 1 {
                    arg_idx += 1;
                    self.args.from_index(arg_idx - 1)
                } else if let Ok(n) = argument.parse::<usize>() {
                    self.args.from_index(n)
                } else {
                    self.args.from_name(argument)
                };

                // format the part before the '{'
                self.format_str[pos..p].fmt(f)?;
                if let Some(a) = pa {
                    a.fmt(f)?;
                } else {
                    // FIXME! this is an error, it should be reported
                    self.format_str[p..=end].fmt(f)?;
                }
                pos = end + 1;
            }
            self.format_str[pos..].fmt(f)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::format;
        use core::fmt::Display;
        #[test]
        fn test_format() {
            assert_eq!(format("Hello", (&[]) as &[String]).to_string(), "Hello");
            assert_eq!(format("Hello {}!", &["world"]).to_string(), "Hello world!");
            assert_eq!(format("Hello {0}!", &["world"]).to_string(), "Hello world!");
            assert_eq!(
                format("Hello -{1}- -{0}-", &[&(40 + 5) as &dyn Display, &"World"]).to_string(),
                "Hello -World- -45-"
            );
            assert_eq!(
                format(
                    format("Hello {{}}!", (&[]) as &[String]).to_string().as_str(),
                    &[format("{}", &["world"])]
                )
                .to_string(),
                "Hello world!"
            );
            assert_eq!(
                format("Hello -{}- -{}-", &[&(40 + 5) as &dyn Display, &"World"]).to_string(),
                "Hello -45- -World-"
            );
            assert_eq!(format("Hello {{0}} {}", &["world"]).to_string(), "Hello {0} world");
        }
    }
}

/// Do the translation and formatting
pub fn translate(
    original: &str,
    _contextid: &str,
    _domain: &str,
    arguments: &(impl FormatArgs + ?Sized),
) -> SharedString {
    use core::fmt::Write;
    let mut output = SharedString::default();
    write!(output, "{}", formatter::format(original, arguments)).unwrap();
    output
}

#[cfg(feature = "ffi")]
mod ffi {
    #![allow(unsafe_code)]
    use super::*;
    use crate::slice::Slice;

    #[no_mangle]
    /// Returns a nul-terminated pointer for this string.
    /// The returned value is owned by the string, and should not be used after any
    /// mutable function have been called on the string, and must not be freed.
    pub extern "C" fn slint_translate(
        to_translate: &mut SharedString,
        context: &SharedString,
        domain: &SharedString,
        arguments: Slice<SharedString>,
    ) {
        *to_translate = translate(to_translate.as_str(), &context, &domain, arguments.as_slice())
    }
}
