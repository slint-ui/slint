// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::SharedString;
use core::fmt::Display;
pub use formatter::FormatArgs;

#[cfg(feature = "tr")]
pub use tr::Translator;

mod formatter {
    use core::fmt::{Display, Formatter, Result};

    pub trait FormatArgs {
        type Output<'a>: Display
        where
            Self: 'a;
        #[allow(clippy::wrong_self_convention)]
        fn from_index(&self, index: usize) -> Option<Self::Output<'_>>;
        #[allow(clippy::wrong_self_convention)]
        fn from_name(&self, _name: &str) -> Option<Self::Output<'_>> {
            None
        }
    }

    impl<T: Display> FormatArgs for [T] {
        type Output<'a>
            = &'a T
        where
            T: 'a;
        fn from_index(&self, index: usize) -> Option<&T> {
            self.get(index)
        }
    }

    impl<const N: usize, T: Display> FormatArgs for [T; N] {
        type Output<'a>
            = &'a T
        where
            T: 'a;
        fn from_index(&self, index: usize) -> Option<&T> {
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

    impl<T: FormatArgs + ?Sized> Display for FormatResult<'_, T> {
        fn fmt(&self, f: &mut Formatter<'_>) -> Result {
            let mut arg_idx = 0;
            let mut pos = 0;
            while let Some(mut p) = self.format_str[pos..].find(['{', '}']) {
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
        use std::string::{String, ToString};
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

struct WithPlural<'a, T: ?Sized>(&'a T, i32);

enum DisplayOrInt<T> {
    Display(T),
    Int(i32),
}
impl<T: Display> Display for DisplayOrInt<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DisplayOrInt::Display(d) => d.fmt(f),
            DisplayOrInt::Int(i) => i.fmt(f),
        }
    }
}

impl<T: FormatArgs + ?Sized> FormatArgs for WithPlural<'_, T> {
    type Output<'b>
        = DisplayOrInt<T::Output<'b>>
    where
        Self: 'b;

    fn from_index(&self, index: usize) -> Option<Self::Output<'_>> {
        self.0.from_index(index).map(DisplayOrInt::Display)
    }

    fn from_name<'b>(&'b self, name: &str) -> Option<Self::Output<'b>> {
        if name == "n" {
            Some(DisplayOrInt::Int(self.1))
        } else {
            self.0.from_name(name).map(DisplayOrInt::Display)
        }
    }
}

/// Do the translation and formatting
pub fn translate(
    original: &str,
    contextid: &str,
    domain: &str,
    arguments: &(impl FormatArgs + ?Sized),
    n: i32,
    plural: &str,
) -> SharedString {
    #![allow(unused)]
    let mut output = SharedString::default();

    // Register a dependency so that language changes trigger a re-evaluation of all relevant bindings
    // and this function is called again.
    #[cfg(any(feature = "tr", all(target_family = "unix", feature = "gettext-rs")))]
    global_translation_property();

    let mut translated: Option<alloc::borrow::Cow<'_, str>> = None;

    #[cfg(feature = "tr")]
    {
        translated = crate::context::GLOBAL_CONTEXT.with(|ctx| {
            let ctx = ctx.get()?;
            let external_translator = ctx.external_translator()?;
            let context = if !contextid.is_empty() { Some(contextid.as_ref()) } else { None };
            Some(
                if plural.is_empty() {
                    external_translator.translate(original, context)
                } else {
                    external_translator.ntranslate(n.try_into().ok()?, original, plural, context)
                }
                .into_owned()
                .into(),
            )
        });
    }

    #[cfg(all(target_family = "unix", feature = "gettext-rs"))]
    if translated.is_none() {
        translated = Some(alloc::borrow::Cow::Owned(translate_gettext(
            original, contextid, domain, n, plural,
        )));
    }

    let translated = translated
        .unwrap_or_else(|| if plural.is_empty() || n == 1 { original } else { plural }.into());

    use core::fmt::Write;
    write!(output, "{}", formatter::format(&translated, &WithPlural(arguments, n))).unwrap();
    output
}

#[cfg(all(target_family = "unix", feature = "gettext-rs"))]
fn translate_gettext(
    string: &str,
    ctx: &str,
    domain: &str,
    n: i32,
    plural: &str,
) -> std::string::String {
    use std::string::String;
    fn mangle_context(ctx: &str, s: &str) -> String {
        std::format!("{ctx}\u{4}{s}")
    }
    fn demangle_context(r: String) -> String {
        if let Some(x) = r.split('\u{4}').last() {
            return x.into();
        }
        r
    }

    if plural.is_empty() {
        if !ctx.is_empty() {
            demangle_context(gettextrs::dgettext(domain, mangle_context(ctx, string)))
        } else {
            gettextrs::dgettext(domain, string)
        }
    } else if !ctx.is_empty() {
        demangle_context(gettextrs::dngettext(
            domain,
            mangle_context(ctx, string),
            mangle_context(ctx, plural),
            n as u32,
        ))
    } else {
        gettextrs::dngettext(domain, string, plural, n as u32)
    }
}

/// Returns the language index and make sure to register a dependency
fn global_translation_property() -> usize {
    crate::context::GLOBAL_CONTEXT.with(|ctx| {
        let Some(ctx) = ctx.get() else { return 0 };
        ctx.0.translations_dirty.as_ref().get()
    })
}

pub fn mark_all_translations_dirty() {
    #[cfg(all(feature = "gettext-rs", target_family = "unix"))]
    {
        // SAFETY: This trick from https://www.gnu.org/software/gettext/manual/html_node/gettext-grok.html
        // is merely incrementing a generational counter that will invalidate gettext's internal cache for translations.
        // If in the worst case it won't invalidate, then old translations are shown.
        #[allow(unsafe_code)]
        unsafe {
            extern "C" {
                static mut _nl_msg_cat_cntr: std::ffi::c_int;
            }
            _nl_msg_cat_cntr += 1;
        }
    }

    crate::context::GLOBAL_CONTEXT.with(|ctx| {
        let Some(ctx) = ctx.get() else { return };
        ctx.0.translations_dirty.mark_dirty();
    })
}

#[cfg(feature = "gettext-rs")]
/// Initialize the translation by calling the [`bindtextdomain`](https://man7.org/linux/man-pages/man3/bindtextdomain.3.html) function from gettext
pub fn gettext_bindtextdomain(_domain: &str, _dirname: std::path::PathBuf) -> std::io::Result<()> {
    #[cfg(target_family = "unix")]
    {
        gettextrs::bindtextdomain(_domain, _dirname)?;
        static START: std::sync::Once = std::sync::Once::new();
        START.call_once(|| {
            gettextrs::setlocale(gettextrs::LocaleCategory::LcAll, "");
        });
        mark_all_translations_dirty();
    }
    Ok(())
}

pub fn translate_from_bundle(
    strs: &[Option<&str>],
    arguments: &(impl FormatArgs + ?Sized),
) -> SharedString {
    let idx = global_translation_property();
    let mut output = SharedString::default();
    let Some(translated) = strs.get(idx).and_then(|x| *x).or_else(|| strs.first().and_then(|x| *x))
    else {
        return output;
    };
    use core::fmt::Write;
    write!(output, "{}", formatter::format(translated, arguments)).unwrap();
    output
}

pub fn translate_from_bundle_with_plural(
    strs: &[Option<&[&str]>],
    plural_rules: &[Option<fn(i32) -> usize>],
    arguments: &(impl FormatArgs + ?Sized),
    n: i32,
) -> SharedString {
    let idx = global_translation_property();
    let mut output = SharedString::default();
    let en = |n| (n != 1) as usize;
    let (translations, rule) = match strs.get(idx) {
        Some(Some(x)) => (x, plural_rules.get(idx).and_then(|x| *x).unwrap_or(en)),
        _ => match strs.first() {
            Some(Some(x)) => (x, plural_rules.first().and_then(|x| *x).unwrap_or(en)),
            _ => return output,
        },
    };
    let Some(translated) = translations.get(rule(n)).or_else(|| translations.first()).cloned()
    else {
        return output;
    };
    use core::fmt::Write;
    write!(output, "{}", formatter::format(translated, &WithPlural(arguments, n))).unwrap();
    output
}

/// This function is called by the generated code to assign the list of bundled languages.
/// Do nothing if the list is already assigned.
pub fn set_bundled_languages(languages: &[&'static str]) {
    crate::context::GLOBAL_CONTEXT.with(|ctx| {
        let Some(ctx) = ctx.get() else { return };
        if ctx.0.translations_bundle_languages.borrow().is_none() {
            ctx.0.translations_bundle_languages.replace(Some(languages.to_vec()));
            #[cfg(feature = "std")]
            if let Some(idx) = index_for_locale(languages) {
                ctx.0.translations_dirty.as_ref().set(idx);
            }
        }
    });
}

/// attempt to select the right bundled translation based on the current locale
#[cfg(feature = "std")]
fn index_for_locale(languages: &[&'static str]) -> Option<usize> {
    let locale = sys_locale::get_locale()?;
    // first, try an exact match
    let idx = languages.iter().position(|x| *x == locale);
    // else, only match the language part
    fn base(l: &str) -> &str {
        l.find(['-', '_', '@']).map_or(l, |i| &l[..i])
    }
    idx.or_else(|| {
        let locale = base(&locale);
        languages.iter().position(|x| base(x) == locale)
    })
}

#[i_slint_core_macros::slint_doc]
/// Select the current translation language when using bundled translations.
///
/// This function requires that the application's `.slint` file was compiled with bundled translations..
/// It must be called after creating the first component.
///
/// The language string is the locale, which matches the name of the folder that contains the `LC_MESSAGES` folder.
/// An empty string or `"en"` will select the default language.
///
/// Returns `Ok` if the language was selected; [`SelectBundledTranslationError`] otherwise.
///
/// See also the [Translation documentation](slint:translations).
pub fn select_bundled_translation(language: &str) -> Result<(), SelectBundledTranslationError> {
    crate::context::GLOBAL_CONTEXT.with(|ctx| {
        let Some(ctx) = ctx.get() else {
            return Err(SelectBundledTranslationError::NoTranslationsBundled);
        };
        let languages = ctx.0.translations_bundle_languages.borrow();
        let Some(languages) = &*languages else {
            return Err(SelectBundledTranslationError::NoTranslationsBundled);
        };
        let idx = languages.iter().position(|x| *x == language);
        if let Some(idx) = idx {
            ctx.0.translations_dirty.as_ref().set(idx);
            Ok(())
        } else if language.is_empty() || language == "en" {
            ctx.0.translations_dirty.as_ref().set(0);
            Ok(())
        } else {
            Err(SelectBundledTranslationError::LanguageNotFound {
                available_languages: languages.iter().map(|x| (*x).into()).collect(),
            })
        }
    })
}

/// Error type returned from the [`select_bundled_translation`] function.
#[derive(Debug)]
pub enum SelectBundledTranslationError {
    /// The language was not found. The list of available languages is included in this error variant.
    LanguageNotFound { available_languages: crate::SharedVector<SharedString> },
    /// There are no bundled translations. Either [`select_bundled_translation`] was called before creating a component,
    /// or the application's `.slint` file was compiled without the bundle translation option.
    NoTranslationsBundled,
}

impl core::fmt::Display for SelectBundledTranslationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SelectBundledTranslationError::LanguageNotFound { available_languages } => {
                write!(f, "The specified language was not found. Available languages are: {available_languages:?}")
            }
            SelectBundledTranslationError::NoTranslationsBundled => {
                write!(f, "There are no bundled translations. Either select_bundled_translation was called before creating a component, or the application's `.slint` file was compiled without the bundle translation option")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for SelectBundledTranslationError {}

#[cfg(feature = "ffi")]
mod ffi {
    #![allow(unsafe_code)]
    use super::*;
    use crate::slice::Slice;

    /// Perform the translation and formatting.
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_translate(
        to_translate: &mut SharedString,
        context: &SharedString,
        domain: &SharedString,
        arguments: Slice<SharedString>,
        n: i32,
        plural: &SharedString,
    ) {
        *to_translate =
            translate(to_translate.as_str(), context, domain, arguments.as_slice(), n, plural)
    }

    /// Mark all translated string as dirty to perform re-translation in case the language change
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_translations_mark_dirty() {
        mark_all_translations_dirty();
    }

    /// Safety: The slice must contain valid null-terminated utf-8 strings
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_translate_from_bundle(
        strs: Slice<*const core::ffi::c_char>,
        arguments: Slice<SharedString>,
        output: &mut SharedString,
    ) {
        *output = SharedString::default();
        let idx = global_translation_property();
        let Some(translated) = strs
            .get(idx)
            .filter(|x| !x.is_null())
            .or_else(|| strs.first())
            .map(|x| core::ffi::CStr::from_ptr(*x).to_str().unwrap())
        else {
            return;
        };
        use core::fmt::Write;
        write!(output, "{}", formatter::format(translated, arguments.as_slice())).unwrap();
    }
    /// strs is all the strings variant of all languages.
    /// indices is the array of indices such that for each language, the corresponding indice is one past the last index of the string for that language.
    /// So to get the string array for that language, one would do `strs[indices[lang-1]..indices[lang]]`
    /// (where indices[-1] is 0)
    ///
    /// Safety; the strs must be pointer to valid null-terminated utf-8 strings
    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn slint_translate_from_bundle_with_plural(
        strs: Slice<*const core::ffi::c_char>,
        indices: Slice<u32>,
        plural_rules: Slice<Option<fn(i32) -> usize>>,
        arguments: Slice<SharedString>,
        n: i32,
        output: &mut SharedString,
    ) {
        *output = SharedString::default();
        let idx = global_translation_property();
        let en = |n| (n != 1) as usize;
        let begin = *indices.get(idx.wrapping_sub(1)).unwrap_or(&0);
        let (translations, rule) = match indices.get(idx) {
            Some(end) if *end != begin => (
                &strs.as_slice()[begin as usize..*end as usize],
                plural_rules.get(idx).and_then(|x| *x).unwrap_or(en),
            ),
            _ => (
                &strs.as_slice()[..*indices.first().unwrap_or(&0) as usize],
                plural_rules.first().and_then(|x| *x).unwrap_or(en),
            ),
        };
        let Some(translated) = translations
            .get(rule(n))
            .or_else(|| translations.first())
            .map(|x| core::ffi::CStr::from_ptr(*x).to_str().unwrap())
        else {
            return;
        };
        use core::fmt::Write;
        write!(output, "{}", formatter::format(translated, &WithPlural(arguments.as_slice(), n)))
            .unwrap();
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_translate_set_bundled_languages(languages: Slice<Slice<'static, u8>>) {
        let languages = languages
            .iter()
            .map(|x| core::str::from_utf8(x.as_slice()).unwrap())
            .collect::<alloc::vec::Vec<_>>();
        set_bundled_languages(&languages);
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_translate_select_bundled_translation(language: Slice<u8>) -> bool {
        let language = core::str::from_utf8(&language).unwrap();
        select_bundled_translation(language).is_ok()
    }
}
