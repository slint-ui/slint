// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore cntr demangle indice
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

/// Do the translation and formatting with the thread's context.
///
/// For callers that have no component to get a context from.
/// Without any context, the string isn't translated, only formatted.
/// See [`crate::SlintContext::translate`].
pub fn translate(
    original: &str,
    contextid: &str,
    domain: &str,
    arguments: &(impl FormatArgs + ?Sized),
    n: i32,
    plural: &str,
) -> SharedString {
    match crate::SlintContext::current() {
        Some(ctx) => ctx.translate(original, contextid, domain, arguments, n, plural),
        None => format_translation(untranslated(original, n, plural), arguments, n),
    }
}

/// The source string to show when there's no translation.
fn untranslated<'a>(original: &'a str, n: i32, plural: &'a str) -> &'a str {
    if plural.is_empty() || n == 1 { original } else { plural }
}

fn format_translation(
    translated: &str,
    arguments: &(impl FormatArgs + ?Sized),
    n: i32,
) -> SharedString {
    let mut output = SharedString::default();
    use core::fmt::Write;
    write!(output, "{}", formatter::format(translated, &WithPlural(arguments, n))).unwrap();
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
        if let Some(x) = r.split('\u{4}').next_back() {
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

/// Mark the translations of the thread's context dirty, so they're translated again.
///
/// For callers that have no component to get a context from.
/// See [`crate::SlintContext::mark_translations_dirty`].
pub fn mark_all_translations_dirty() {
    match crate::SlintContext::current() {
        Some(ctx) => ctx.mark_translations_dirty(),
        None => invalidate_gettext_cache(),
    }
}

/// gettext's cache is process-wide, so this affects every context.
fn invalidate_gettext_cache() {
    // _nl_msg_cat_cntr is defined by glibc and the standalone GNU libintl, but not by musl,
    // which provides its own gettext implementation without that cache counter.
    #[cfg(all(feature = "gettext-rs", target_family = "unix", not(target_env = "musl")))]
    {
        // SAFETY: This trick from https://www.gnu.org/software/gettext/manual/html_node/gettext-grok.html
        // is merely incrementing a generational counter that will invalidate gettext's internal cache for translations.
        // If in the worst case it won't invalidate, then old translations are shown.
        #[allow(unsafe_code)]
        unsafe {
            unsafe extern "C" {
                static mut _nl_msg_cat_cntr: std::ffi::c_int;
            }
            _nl_msg_cat_cntr += 1;
        }
    }
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

/// The name and the decimal separator of a bundled language, as generated by the compiler
/// when bundling the translations.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TranslationsBundled {
    pub language: crate::slice::Slice<'static, u8>,
    pub decimal_separator: char,
}

/// The languages of the bundled translations, in the order in which they were bundled.
pub(crate) enum BundledLanguages {
    /// The array generated by the compiler, which lives in the binary.
    Static(&'static [TranslationsBundled]),
    /// The languages of a bundle that the interpreter made at runtime.
    Dynamic(alloc::vec::Vec<(alloc::string::String, char)>),
}

impl BundledLanguages {
    fn len(&self) -> usize {
        match self {
            Self::Static(l) => l.len(),
            Self::Dynamic(l) => l.len(),
        }
    }

    fn get(&self, index: usize) -> Option<(&str, char)> {
        match self {
            Self::Static(l) => l.get(index).map(|x| {
                (
                    core::str::from_utf8(x.language.as_slice()).unwrap_or_default(),
                    x.decimal_separator,
                )
            }),
            Self::Dynamic(l) => l.get(index).map(|x| (x.0.as_str(), x.1)),
        }
    }

    /// The name and decimal separator of each language, in the order in which they were bundled.
    fn iter(&self) -> impl Iterator<Item = (&str, char)> {
        (0..self.len()).filter_map(|i| self.get(i))
    }
}

/// Assign the languages to the given context, unless it already has some.
/// The list is only built when it is needed.
fn set_bundled_languages_impl(
    ctx: &crate::SlintContext,
    translations: impl FnOnce() -> BundledLanguages,
) {
    if ctx.0.translations_bundle.borrow().is_none() {
        let translations = translations();
        #[cfg(feature = "std")]
        if let Some(idx) = language_index_from_sys_locale(&translations) {
            ctx.0.as_ref().project_ref().translations_dirty.set(idx);
        }
        ctx.0.translations_bundle.replace(Some(translations));
    }
}

/// attempt to select the right bundled translation based on the current system locale
#[cfg(feature = "std")]
fn language_index_from_sys_locale(languages: &BundledLanguages) -> Option<usize> {
    let locale = sys_locale::get_locale()?;
    // first, try an exact match
    let idx = languages.iter().position(|(l, _)| l == locale);
    // else, only match the language part
    fn base(l: &str) -> &str {
        l.find(['-', '_', '@']).map_or(l, |i| &l[..i])
    }
    idx.or_else(|| {
        let locale = base(&locale);
        languages.iter().position(|(l, _)| base(l) == locale)
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
    crate::SlintContext::current()
        .ok_or(SelectBundledTranslationError::NoTranslationsBundled)?
        .select_bundled_translation(language)
}

impl crate::SlintContext {
    /// The index of the selected bundled language.
    /// Registers a dependency, so a binding that reads it is evaluated again when the language
    /// changes or the translations are marked dirty.
    fn language_index(&self) -> usize {
        self.0.as_ref().project_ref().translations_dirty.get()
    }

    /// Translate and format a string with this context's translator and language.
    pub fn translate(
        &self,
        original: &str,
        contextid: &str,
        domain: &str,
        arguments: &(impl FormatArgs + ?Sized),
        n: i32,
        plural: &str,
    ) -> SharedString {
        #![allow(unused)]
        // Register a dependency so that language changes trigger a re-evaluation of all relevant bindings
        // and this function is called again.
        #[cfg(any(feature = "tr", all(target_family = "unix", feature = "gettext-rs")))]
        self.language_index();

        let mut translated: Option<alloc::borrow::Cow<'_, str>> = None;

        #[cfg(feature = "tr")]
        if let Some(external_translator) = self.external_translator() {
            let context = if !contextid.is_empty() { Some(contextid) } else { None };
            translated = if plural.is_empty() {
                Some(external_translator.translate(original, context).into_owned().into())
            } else {
                n.try_into().ok().map(|n| {
                    external_translator.ntranslate(n, original, plural, context).into_owned().into()
                })
            };
        }

        #[cfg(all(target_family = "unix", feature = "gettext-rs"))]
        if translated.is_none() {
            translated = Some(alloc::borrow::Cow::Owned(translate_gettext(
                original, contextid, domain, n, plural,
            )));
        }

        let translated = translated.unwrap_or_else(|| untranslated(original, n, plural).into());
        format_translation(&translated, arguments, n)
    }

    /// Translate a string bundled into the application, in this context's language.
    /// Falls back to the default language if the selected one has no translation.
    ///
    /// `strs` holds the string in each bundled language.
    pub fn translate_from_bundle<S: AsRef<str>>(
        &self,
        strs: &[Option<S>],
        arguments: &(impl FormatArgs + ?Sized),
    ) -> SharedString {
        let idx = self.language_index();
        let mut output = SharedString::default();
        let Some(translated) = strs
            .get(idx)
            .and_then(|x| x.as_ref())
            .or_else(|| strs.first().and_then(|x| x.as_ref()))
        else {
            return output;
        };
        use core::fmt::Write;
        write!(output, "{}", formatter::format(translated.as_ref(), arguments)).unwrap();
        output
    }

    /// Translate a string with plural forms bundled into the application, in this context's
    /// language.
    ///
    /// `strs` holds the plural forms in each bundled language,
    /// and `plural_rules` the rule that picks the form for each language.
    pub fn translate_from_bundle_with_plural(
        &self,
        strs: &[Option<&[&str]>],
        plural_rules: &[Option<fn(i32) -> usize>],
        arguments: &(impl FormatArgs + ?Sized),
        n: i32,
    ) -> SharedString {
        self.translate_from_bundle_with_plural_form(
            strs,
            |language_index| plural_rules.get(language_index).and_then(|x| *x).map(|rule| rule(n)),
            arguments,
            n,
        )
    }

    /// Same as [`Self::translate_from_bundle_with_plural`], but `plural_form` computes the form
    /// from the language index.
    /// It returns `None` if the language has no rule, in which case the English rule is used.
    pub fn translate_from_bundle_with_plural_form<S: AsRef<str>>(
        &self,
        strs: &[Option<&[S]>],
        plural_form: impl FnOnce(usize) -> Option<usize>,
        arguments: &(impl FormatArgs + ?Sized),
        n: i32,
    ) -> SharedString {
        let idx = self.language_index();
        let mut output = SharedString::default();
        let en = |n| (n != 1) as usize;
        let (translations, form) = match strs.get(idx) {
            Some(Some(x)) => (x, plural_form(idx)),
            _ => match strs.first() {
                Some(Some(x)) => (x, plural_form(0)),
                _ => return output,
            },
        };
        let Some(translated) =
            translations.get(form.unwrap_or_else(|| en(n))).or_else(|| translations.first())
        else {
            return output;
        };
        use core::fmt::Write;
        write!(output, "{}", formatter::format(translated.as_ref(), &WithPlural(arguments, n)))
            .unwrap();
        output
    }

    /// Assign the list of bundled languages and their decimal separator to this context,
    /// and select the one that matches the system locale.
    ///
    /// Does nothing if this context already has a list, so that a language selected with
    /// [`Self::select_bundled_translation`] survives a re-instantiation.
    pub fn set_bundled_languages(
        &self,
        languages: impl IntoIterator<Item = (alloc::string::String, char)>,
    ) {
        set_bundled_languages_impl(self, || {
            BundledLanguages::Dynamic(languages.into_iter().collect())
        });
    }

    /// Assign the languages the compiler bundled into the application, unless this context
    /// already has some. See [`Self::set_bundled_languages`].
    #[doc(hidden)]
    pub fn set_static_bundled_languages(&self, translations: &'static [TranslationsBundled]) {
        set_bundled_languages_impl(self, || BundledLanguages::Static(translations));
    }

    /// Select this context's language when using bundled translations.
    /// See [`select_bundled_translation`].
    pub fn select_bundled_translation(
        &self,
        language: &str,
    ) -> Result<(), SelectBundledTranslationError> {
        let translations = self.0.translations_bundle.borrow();
        let Some(translations) = &*translations else {
            return Err(SelectBundledTranslationError::NoTranslationsBundled);
        };
        let pinned = self.0.as_ref().project_ref();
        if let Some((idx, (_, decimal_separator))) =
            translations.iter().enumerate().find(|(_, (l, _))| *l == language)
        {
            pinned.translations_dirty.as_ref().set(idx);
            pinned.locale_decimal_separator.as_ref().set(decimal_separator);
            Ok(())
        } else if language.is_empty() || language == "en" {
            pinned.translations_dirty.as_ref().set(0);
            pinned.locale_decimal_separator.as_ref().set(i_slint_common::DEFAULT_DECIMAL_SEPARATOR);
            Ok(())
        } else {
            Err(SelectBundledTranslationError::LanguageNotFound {
                available_languages: translations.iter().map(|(l, _)| l.into()).collect(),
            })
        }
    }

    /// Translate this context's strings again, for example after the translation files changed.
    pub fn mark_translations_dirty(&self) {
        invalidate_gettext_cache();
        let pinned = self.0.as_ref().project_ref();
        pinned.translations_dirty.mark_dirty();

        #[cfg(all(feature = "gettext-rs", target_family = "unix"))]
        if let Some(locale) = sys_locale::get_locale() {
            pinned
                .locale_decimal_separator
                .set(i_slint_common::decimal_separator_for_locale(&locale))
        }
    }
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
                write!(
                    f,
                    "The specified language was not found. Available languages are: {available_languages:?}"
                )
            }
            SelectBundledTranslationError::NoTranslationsBundled => {
                write!(
                    f,
                    "There are no bundled translations. Either select_bundled_translation was called before creating a component, or the application's `.slint` file was compiled without the bundle translation option"
                )
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

    fn current_language_index() -> usize {
        crate::SlintContext::current().map_or(0, |ctx| ctx.language_index())
    }

    /// return the current decimal-separator for the `Platform.decimal-separator` property
    #[unsafe(no_mangle)]
    pub extern "C" fn slint_decimal_separator(out: &mut SharedString) {
        *out = crate::SharedString::from(crate::string::current_decimal_separator())
    }

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
        let idx = current_language_index();
        let Some(translated) = strs
            .get(idx)
            .filter(|x| !x.is_null())
            .or_else(|| strs.first())
            .map(|x| unsafe { core::ffi::CStr::from_ptr(*x) }.to_str().unwrap())
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
        let idx = current_language_index();
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
            .map(|x| unsafe { core::ffi::CStr::from_ptr(*x) }.to_str().unwrap())
        else {
            return;
        };
        use core::fmt::Write;
        write!(output, "{}", formatter::format(translated, &WithPlural(arguments.as_slice(), n)))
            .unwrap();
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_translate_set_bundled_languages(
        languages: Slice<'static, TranslationsBundled>,
    ) {
        if let Some(ctx) = crate::SlintContext::current() {
            ctx.set_static_bundled_languages(languages.as_slice());
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn slint_translate_select_bundled_translation(language: Slice<u8>) -> bool {
        let Ok(language) = core::str::from_utf8(&language) else { return false };
        select_bundled_translation(language).is_ok()
    }
}

#[test]
#[cfg(feature = "std")]
fn test_bundled_translations_per_context() {
    use crate::testing::NoWindowPlatform as TestPlatform;
    let languages = || [("".into(), '.'), ("fr".into(), ',')];
    let strs = [Some("Hello"), Some("Bonjour")];
    let no_args: &[SharedString] = &[];

    // The first context created becomes the thread's.
    let thread_ctx = crate::SlintContext::new(alloc::boxed::Box::new(TestPlatform));
    thread_ctx.set_bundled_languages(languages());
    let other = crate::SlintContext::new(alloc::boxed::Box::new(TestPlatform));
    other.set_bundled_languages(languages());
    thread_ctx.select_bundled_translation("").unwrap();
    other.select_bundled_translation("").unwrap();

    other.select_bundled_translation("fr").unwrap();
    assert_eq!(other.translate_from_bundle(&strs, no_args), "Bonjour");
    assert_eq!(other.locale_decimal_separator(), ',');
    assert_eq!(thread_ctx.translate_from_bundle(&strs, no_args), "Hello");
    assert_eq!(thread_ctx.locale_decimal_separator(), '.');

    select_bundled_translation("fr").unwrap();
    assert_eq!(thread_ctx.translate_from_bundle(&strs, no_args), "Bonjour");
    other.select_bundled_translation("en").unwrap();
    assert_eq!(other.translate_from_bundle(&strs, no_args), "Hello");

    assert!(matches!(
        other.select_bundled_translation("de"),
        Err(SelectBundledTranslationError::LanguageNotFound { .. })
    ));
}
