// FIXME: make .logger() return type uniform

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use crate::{
        error::{IncludedRangesError, LanguageError, ParserError},
        language::{Language, LanguageRef},
        logger::{Logger, LoggerReturn},
        point::Point,
        range::Range,
        tree::Tree,
    };
    use std::convert::TryFrom;

    pub struct Parser {
        inner: tree_sitter::Parser,
    }

    impl Parser {
        #[inline]
        pub fn new() -> Result<Self, ParserError> {
            let inner = tree_sitter::Parser::new();
            Ok(Self { inner })
        }

        #[inline]
        pub fn language(&self) -> Option<LanguageRef<'_>> {
            self.inner.language().map(Into::into)
        }

        #[inline]
        pub fn logger(&self) -> Option<LoggerReturn<'_, '_>> {
            self.inner.logger().map(LoggerReturn::new)
        }

        #[inline]
        pub fn parse(
            &mut self,
            text: impl AsRef<[u8]>,
            old_tree: Option<&Tree>,
        ) -> Result<Option<Tree>, ParserError> {
            let old_tree = old_tree.map(|tree| &tree.inner);
            Ok(self.inner.parse(text, old_tree).map(Into::into))
        }

        #[inline]
        pub fn parse_utf16(
            &mut self,
            text: impl AsRef<[u16]>,
            old_tree: Option<&Tree>,
        ) -> Result<Option<Tree>, ParserError> {
            let old_tree = old_tree.map(|tree| &tree.inner);
            Ok(self.inner.parse_utf16_le(text, old_tree).map(Into::into))
        }

        #[inline]
        pub fn parse_utf16_with<T>(
            &mut self,
            mut callback: impl FnMut(u32, Point) -> T,
            old_tree: Option<&Tree>,
        ) -> Result<Option<Tree>, ParserError>
        where
            T: AsRef<[u16]>,
        {
            let mut callback =
                |offset, inner| callback(u32::try_from(offset).unwrap(), Point { inner });
            let old_tree = old_tree.map(|tree| &tree.inner);
            Ok(self
                .inner
                .parse_utf16_le_with_options(&mut callback, old_tree, None)
                .map(Into::into))
        }

        #[inline]
        pub fn parse_with<T>(
            &mut self,
            mut callback: impl FnMut(u32, Point) -> T + 'static,
            old_tree: Option<&Tree>,
        ) -> Result<Option<Tree>, ParserError>
        where
            T: AsRef<[u8]>,
        {
            let mut callback =
                |offset, inner| callback(u32::try_from(offset).unwrap(), Point { inner });
            let old_tree = old_tree.map(|tree| &tree.inner);
            Ok(self
                .inner
                .parse_with_options(&mut callback, old_tree, None)
                .map(Into::into))
        }

        #[cfg(unix)]
        #[inline]
        pub fn print_dot_graphs(&mut self, file: &impl std::os::unix::io::AsRawFd) {
            self.inner.print_dot_graphs(file)
        }

        #[inline]
        pub fn reset(&mut self) {
            self.inner.reset()
        }

        #[inline]
        pub fn set_included_ranges(&mut self, ranges: &[Range]) -> Result<(), IncludedRangesError> {
            let ranges = ranges.iter().map(|range| range.inner).collect::<Vec<_>>();
            self.inner.set_included_ranges(&ranges).map_err(Into::into)
        }

        #[inline]
        pub fn set_language(&mut self, language: &Language) -> Result<(), LanguageError> {
            self.inner.set_language(&language.inner).map_err(Into::into)
        }

        #[inline]
        pub fn set_logger(&mut self, logger: Option<Logger<'static>>) {
            self.inner.set_logger(logger)
        }

        #[inline]
        pub fn stop_printing_dot_graphs(&mut self) {
            self.inner.stop_printing_dot_graphs()
        }
    }

    impl From<tree_sitter::Parser> for Parser {
        #[inline]
        fn from(inner: tree_sitter::Parser) -> Self {
            Self { inner }
        }
    }

    impl std::panic::RefUnwindSafe for Parser {}

    unsafe impl Send for Parser {}

    impl Unpin for Parser {}

    impl std::panic::UnwindSafe for Parser {}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;

#[cfg(target_arch = "wasm32")]
mod wasm {
    use crate::{
        error::{IncludedRangesError, LanguageError, ParserError},
        language::Language,
        logger::{LogType, Logger, LoggerReturn},
        point::Point,
        range::Range,
        tree::Tree,
    };
    use js_sys::{Function, JsString};
    use wasm_bindgen::{JsCast, prelude::*};

    pub struct Parser {
        inner: topiary_web_tree_sitter_sys::Parser,
        options: topiary_web_tree_sitter_sys::ParseOptions,
    }

    unsafe impl Send for Parser {}

    impl Parser {
        #[inline]
        pub fn new() -> Result<Self, ParserError> {
            let inner = topiary_web_tree_sitter_sys::Parser::new()?;
            let options = Default::default();
            Ok(Self { inner, options })
        }

        #[inline]
        pub fn language(&self) -> Option<Language> {
            self.inner.get_language().map(Into::into)
        }

        #[inline]
        pub fn logger(&self) -> Option<LoggerReturn> {
            if let Some(logger) = self.inner.get_logger() {
                let options = js_sys::Object::new().into();
                let fun = Box::new(move |type_: LogType, message: JsString| {
                    let context = &wasm_bindgen::JsValue::NULL;
                    let arg0 = &type_.into();
                    let arg1 = &options;
                    let arg2 = &message.into();
                    logger.call3(context, arg0, arg1, arg2).unwrap();
                }) as Box<dyn FnMut(LogType, JsString)>;
                Some(LoggerReturn::new(fun))
            } else {
                None
            }
        }

        #[inline]
        pub fn parse(
            &mut self,
            text: impl AsRef<[u8]>,
            old_tree: Option<&Tree>,
        ) -> Result<Option<Tree>, ParserError> {
            let text = text.as_ref();
            let text = unsafe { std::str::from_utf8_unchecked(text) };
            let text = &text.into();
            let old_tree = old_tree.map(|tree| &tree.inner);
            let options = Some(&self.options);
            self.inner
                .parse_with_string(text, old_tree, options)
                .map(|ok| ok.map(Into::into))
                .map_err(Into::into)
        }

        // #[inline]
        // pub fn parse_utf16(
        //     &mut self,
        //     text: impl AsRef<[u16]>,
        //     old_tree: Option<&Tree>,
        // ) -> Result<Option<Tree>, ParserError> {
        //     unimplemented!()
        // }

        // #[inline]
        // pub fn parse_utf16_with<T>(
        //     &mut self,
        //     callback: impl FnMut(u32, Point) -> T + 'static,
        //     old_tree: Option<&Tree>,
        // ) -> Result<Option<Tree>, ParserError>
        // where
        //     T: AsRef<[u16]>,
        // {
        //     unimplemented!()
        // }

        #[inline]
        pub fn parse_with<T>(
            &mut self,
            mut callback: impl FnMut(u32, Option<Point>, Option<u32>) -> T + 'static,
            old_tree: Option<&Tree>,
        ) -> Result<Option<Tree>, ParserError>
        where
            T: AsRef<[u8]>,
        {
            let closure = Closure::wrap(Box::new(
                move |start_index, start_point: Option<_>, end_index| {
                    let start_point = start_point.map(Into::into);
                    let result = callback(start_index, start_point, end_index);
                    let result = result.as_ref();
                    let result = unsafe { std::str::from_utf8_unchecked(result) };
                    Some(result.into())
                },
            )
                as Box<
                    dyn FnMut(
                        u32,
                        Option<topiary_web_tree_sitter_sys::Point>,
                        Option<u32>,
                    ) -> Option<JsString>,
                >);
            let input = closure.as_ref().unchecked_ref();
            let old_tree = old_tree.map(|tree| &tree.inner);
            let options = Some(&self.options);
            let result = self
                .inner
                .parse_with_function(input, old_tree, options)
                .map(|ok| ok.map(Into::into))
                .map_err(Into::into);
            closure.forget();
            result
        }

        #[inline]
        pub fn reset(&mut self) {
            self.inner.reset()
        }

        #[inline]
        pub fn set_included_ranges(&mut self, ranges: &[Range]) -> Result<(), IncludedRangesError> {
            // FIXME: check `ranges[i].end_byte <= ranges[i + 1].start_byte` or throw
            let ranges = ranges
                .iter()
                .map(|range| &range.inner)
                .collect::<js_sys::Array>();
            let options = topiary_web_tree_sitter_sys::ParseOptions::new(Some(&ranges));
            self.options = options;
            Ok(())
        }

        #[inline]
        pub fn set_language(&mut self, language: &Language) -> Result<(), LanguageError> {
            let language = Some(&language.inner);
            self.inner.set_language(language).map_err(Into::into)
        }

        #[inline]
        pub fn set_logger(&mut self, logger: Option<Logger<'static>>) {
            if let Some(logger) = logger {
                let clo = Closure::wrap(logger);
                let fun = clo.as_ref().unchecked_ref::<Function>();
                self.inner.set_logger(Some(fun));
                clo.forget();
            } else {
                self.inner.set_logger(None);
            }
        }

        #[inline]
        pub fn set_timeout_micros(&mut self, timeout_micros: f64) {
            self.inner.set_timeout_micros(timeout_micros)
        }

        // #[inline]
        // pub fn stop_printing_dot_graphs(&mut self) {
        //     unimplemented!()
        // }

        #[inline]
        pub fn timeout_micros(&self) -> f64 {
            self.inner.get_timeout_micros()
        }
    }

    impl Drop for Parser {
        #[inline]
        fn drop(&mut self) {
            self.inner.delete();
        }
    }

    impl From<topiary_web_tree_sitter_sys::Parser> for Parser {
        #[inline]
        fn from(inner: topiary_web_tree_sitter_sys::Parser) -> Self {
            let options = Default::default();
            Self { inner, options }
        }
    }

    impl std::panic::RefUnwindSafe for Parser {}

    impl Unpin for Parser {}

    impl std::panic::UnwindSafe for Parser {}
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
