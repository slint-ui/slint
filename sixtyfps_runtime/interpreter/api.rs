/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use core::convert::TryInto;
use sixtyfps_corelib::{Brush, ImageReference, PathData, SharedString, SharedVector};
use std::collections::HashMap;
use std::iter::FromIterator;
use std::path::{Path, PathBuf};
use std::rc::Rc;

#[doc(inline)]
pub use sixtyfps_compilerlib::diagnostics::{Diagnostic, DiagnosticLevel};

/// This is a dynamically typed value used in the SixtyFPS interpreter.
/// It can hold a value of different types, and you should use the
/// [`From`] or [`TryInto`] traits to access the value.
///
/// ```
/// # use sixtyfps_interpreter::*;
/// use core::convert::TryInto;
/// // create a value containing an integer
/// let v = Value::from(100u32);
/// assert_eq!(v.try_into(), Ok(100u32));
/// ```
#[derive(Clone)]
#[non_exhaustive]
#[repr(C)]
pub enum Value {
    /// There is nothing in this value. That's the default.
    /// For example, a function that do not return a result would return a Value::Void
    Void,
    /// An `int` or a `float` (this is also used for unit based type such as `length` or `angle`)
    Number(f64),
    /// Correspond to the `string` type in .60
    String(SharedString),
    /// Correspond to the `bool` type in .60
    Bool(bool),
    /// Correspond to the `image` type in .60
    Image(ImageReference),
    /// An Array in the .60 language.
    Array(SharedVector<Value>),
    /// A more complex model which is not created by the interpreter itself (Value::Array can also be used for model)
    Model(Rc<dyn sixtyfps_corelib::model::Model<Data = Value>>),
    /// An object
    Struct(Struct),
    /// Corresespond to `brush` or `color` type in .60.  For color, this is then a [`Brush::SolidColor`]
    Brush(Brush),
    #[doc(hidden)]
    /// The elements of a path
    PathElements(PathData),
    #[doc(hidden)]
    /// An easing curve
    EasingCurve(sixtyfps_corelib::animations::EasingCurve),
    #[doc(hidden)]
    /// An enumation, like TextHorizontalAlignment::align_center
    EnumerationValue(String, String),
}

impl Default for Value {
    fn default() -> Self {
        Value::Void
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Value::Void => matches!(other, Value::Void),
            Value::Number(lhs) => matches!(other, Value::Number(rhs) if lhs == rhs),
            Value::String(lhs) => matches!(other, Value::String(rhs) if lhs == rhs),
            Value::Bool(lhs) => matches!(other, Value::Bool(rhs) if lhs == rhs),
            Value::Image(lhs) => matches!(other, Value::Image(rhs) if lhs == rhs),
            Value::Array(lhs) => matches!(other, Value::Array(rhs) if lhs == rhs),
            Value::Model(lhs) => matches!(other, Value::Model(rhs) if Rc::ptr_eq(lhs, rhs)),
            Value::Struct(lhs) => matches!(other, Value::Struct(rhs) if lhs == rhs),
            Value::Brush(lhs) => matches!(other, Value::Brush(rhs) if lhs == rhs),
            Value::PathElements(lhs) => matches!(other, Value::PathElements(rhs) if lhs == rhs),
            Value::EasingCurve(lhs) => matches!(other, Value::EasingCurve(rhs) if lhs == rhs),
            Value::EnumerationValue(lhs_name, lhs_value) => {
                matches!(other, Value::EnumerationValue(rhs_name, rhs_value) if lhs_name == rhs_name && lhs_value == rhs_value)
            }
        }
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Void => write!(f, "Value::Void"),
            Value::Number(n) => write!(f, "Value::Number({:?})", n),
            Value::String(s) => write!(f, "Value::String({:?})", s),
            Value::Bool(b) => write!(f, "Value::Bool({:?})", b),
            Value::Image(i) => write!(f, "Value::Image({:?})", i),
            Value::Array(a) => write!(f, "Value::Array({:?})", a),
            Value::Model(_) => write!(f, "Value::Model(<model object>)"),
            Value::Struct(s) => write!(f, "Value::Struct({:?})", s),
            Value::Brush(b) => write!(f, "Value::Brush({:?})", b),
            Value::PathElements(e) => write!(f, "Value::PathElements({:?})", e),
            Value::EasingCurve(c) => write!(f, "Value::EasingCurve({:?})", c),
            Value::EnumerationValue(n, v) => write!(f, "Value::EnumerationValue({:?}, {:?})", n, v),
        }
    }
}

/// Helper macro to implement the From / TryInto for Value
///
/// For example
/// `declare_value_conversion!(Number => [u32, u64, i32, i64, f32, f64] );`
/// means that `Value::Number` can be converted to / from each of the said rust types
///
/// For `Value::Object` mapping to a rust `struct`, one can use [`declare_value_struct_conversion!`]
/// And for `Value::EnumerationValue` which maps to a rust `enum`, one can use [`declare_value_struct_conversion!`]
macro_rules! declare_value_conversion {
    ( $value:ident => [$($ty:ty),*] ) => {
        $(
            impl From<$ty> for Value {
                fn from(v: $ty) -> Self {
                    Value::$value(v as _)
                }
            }
            impl TryInto<$ty> for Value {
                type Error = Value;
                fn try_into(self) -> Result<$ty, Value> {
                    match self {
                        //Self::$value(x) => x.try_into().map_err(|_|()),
                        Self::$value(x) => Ok(x as _),
                        _ => Err(self)
                    }
                }
            }
        )*
    };
}
declare_value_conversion!(Number => [u32, u64, i32, i64, f32, f64, usize, isize] );
declare_value_conversion!(String => [SharedString] );
declare_value_conversion!(Bool => [bool] );
declare_value_conversion!(Image => [ImageReference] );
declare_value_conversion!(Struct => [Struct] );
declare_value_conversion!(Brush => [Brush] );
declare_value_conversion!(PathElements => [PathData]);
declare_value_conversion!(EasingCurve => [sixtyfps_corelib::animations::EasingCurve]);

/// Implement From / TryInto for Value that convert a `struct` to/from `Value::Object`
macro_rules! declare_value_struct_conversion {
    (struct $name:path { $($field:ident),* $(,)? }) => {
        impl From<$name> for Value {
            fn from($name { $($field),* }: $name) -> Self {
                let mut struct_ = Struct::default();
                $(struct_.set_field(stringify!($field).into(), $field.into());)*
                Value::Struct(struct_)
            }
        }
        impl TryInto<$name> for Value {
            type Error = ();
            fn try_into(self) -> Result<$name, ()> {
                match self {
                    Self::Struct(x) => {
                        type Ty = $name;
                        Ok(Ty {
                            $($field: x.get_field(stringify!($field)).ok_or(())?.clone().try_into().map_err(|_|())?),*
                        })
                    }
                    _ => Err(()),
                }
            }
        }
    };
}

declare_value_struct_conversion!(struct sixtyfps_corelib::model::StandardListViewItem { text });
declare_value_struct_conversion!(struct sixtyfps_corelib::properties::StateInfo { current_state, previous_state, change_time });
declare_value_struct_conversion!(struct sixtyfps_corelib::input::KeyboardModifiers { control, alt, shift, meta });
declare_value_struct_conversion!(struct sixtyfps_corelib::input::KeyEvent { event_type, text, modifiers });

/// Implement From / TryInto for Value that convert an `enum` to/from `Value::EnumerationValue`
///
/// The `enum` must derive `Display` and `FromStr`
/// (can be done with `strum_macros::EnumString`, `strum_macros::Display` derive macro)
macro_rules! declare_value_enum_conversion {
    ($ty:ty, $n:ident) => {
        impl From<$ty> for Value {
            fn from(v: $ty) -> Self {
                Value::EnumerationValue(stringify!($n).to_owned(), v.to_string())
            }
        }
        impl TryInto<$ty> for Value {
            type Error = ();
            fn try_into(self) -> Result<$ty, ()> {
                use std::str::FromStr;
                match self {
                    Self::EnumerationValue(enumeration, value) => {
                        if enumeration != stringify!($n) {
                            return Err(());
                        }

                        <$ty>::from_str(value.as_str()).map_err(|_| ())
                    }
                    _ => Err(()),
                }
            }
        }
    };
}

declare_value_enum_conversion!(
    sixtyfps_corelib::items::TextHorizontalAlignment,
    TextHorizontalAlignment
);
declare_value_enum_conversion!(
    sixtyfps_corelib::items::TextVerticalAlignment,
    TextVerticalAlignment
);
declare_value_enum_conversion!(sixtyfps_corelib::items::TextOverflow, TextOverflow);
declare_value_enum_conversion!(sixtyfps_corelib::items::TextWrap, TextWrap);
declare_value_enum_conversion!(sixtyfps_corelib::layout::LayoutAlignment, LayoutAlignment);
declare_value_enum_conversion!(sixtyfps_corelib::items::ImageFit, ImageFit);
declare_value_enum_conversion!(sixtyfps_corelib::input::KeyEventType, KeyEventType);
declare_value_enum_conversion!(sixtyfps_corelib::items::EventResult, EventResult);
declare_value_enum_conversion!(sixtyfps_corelib::items::FillRule, FillRule);

impl From<sixtyfps_corelib::animations::Instant> for Value {
    fn from(value: sixtyfps_corelib::animations::Instant) -> Self {
        Value::Number(value.0 as _)
    }
}
impl TryInto<sixtyfps_corelib::animations::Instant> for Value {
    type Error = ();
    fn try_into(self) -> Result<sixtyfps_corelib::animations::Instant, ()> {
        match self {
            Value::Number(x) => Ok(sixtyfps_corelib::animations::Instant(x as _)),
            _ => Err(()),
        }
    }
}

impl From<()> for Value {
    #[inline]
    fn from(_: ()) -> Self {
        Value::Void
    }
}
impl TryInto<()> for Value {
    type Error = ();
    #[inline]
    fn try_into(self) -> Result<(), ()> {
        Ok(())
    }
}

impl From<sixtyfps_corelib::Color> for Value {
    #[inline]
    fn from(c: sixtyfps_corelib::Color) -> Self {
        Value::Brush(Brush::SolidColor(c))
    }
}
impl TryInto<sixtyfps_corelib::Color> for Value {
    type Error = Value;
    #[inline]
    fn try_into(self) -> Result<sixtyfps_corelib::Color, Value> {
        match self {
            Value::Brush(Brush::SolidColor(c)) => Ok(c),
            _ => Err(self),
        }
    }
}

/// This type represent a runtime instance of structure in `.60`.
///
/// This can either be an instance of a name structure introduced
/// with the `struct` keywrod in the .60 file, or an annonymous struct
/// writen with the `{ key: value, }`  notation.
///
/// It can be constructed with the [`FromIterator`] trait, and converted
/// into or from a [`Value`] with the [`From`] and [`TryInto`] trait
///
///
/// ```
/// # use sixtyfps_interpreter::*;
/// use core::convert::TryInto;
/// // Construct a value from a key/value iterator
/// let value : Value = [("foo".into(), 45u32.into()), ("bar".into(), true.into())]
///     .iter().cloned().collect::<Struct>().into();
///
/// // get the properties of a `{ foo: 45, bar: true }`
/// let s : Struct = value.try_into().unwrap();
/// assert_eq!(s.get_field("foo").unwrap().try_into(), Ok(45u32));
/// ```
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Struct(HashMap<String, Value>);
impl Struct {
    /// Get the value for a given struct field
    pub fn get_field(&self, name: &str) -> Option<Value> {
        self.0.get(name).cloned()
    }
    /// Set the value of a given struct field
    pub fn set_field(&mut self, name: String, value: Value) {
        self.0.insert(name, value);
    }

    /// Iterate over all the fields in this struct
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Value)> {
        self.0.iter().map(|(a, b)| (a.as_str(), b))
    }
}

impl FromIterator<(String, Value)> for Struct {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// FIXME: use SharedArray instead?
impl From<Vec<Value>> for Value {
    fn from(a: Vec<Value>) -> Self {
        Value::Array(a.into_iter().collect())
    }
}
impl TryInto<Vec<Value>> for Value {
    type Error = Value;
    fn try_into(self) -> Result<Vec<Value>, Value> {
        if let Value::Array(a) = self {
            Ok(a.into_iter().collect())
        } else {
            Err(self)
        }
    }
}

/// ComponentDescription is a representation of a compiled component from .60
///
/// It can be constructed from a .60 file using the [`Self::from_path`] or [`Self::from_source`] functions.
/// And then it can be instentiated with the [`Self::create`] function
#[derive(Clone)]
pub struct ComponentDefinition {
    inner: Rc<crate::dynamic_component::ComponentDescription<'static>>,
}

impl ComponentDefinition {
    /// Compile a .60 file into a ComponentDefinition
    ///
    /// The first element of the returned tuple is going to be the compiled
    /// `ComponentDefinition` if there was no errors. This function also return
    /// a vector if diagnostics with errors and/or warnings.
    /// The [`print_diagnostics`] function can be used to display the diagnostics
    /// to the users.
    ///
    /// This function is `async` but in practice, this is only asynchronious if
    /// [`CompilerConfiguration::with_file_loader`] is set and its future is actually asynchronious.
    /// If that is not used, then it is fine to use a very simple executor, such as the one
    /// provided by the `spin_on` crate
    pub async fn from_path<P: AsRef<Path>>(
        path: P,
        config: CompilerConfiguration,
    ) -> (Option<Self>, Vec<Diagnostic>) {
        let path = path.as_ref();
        let source = match sixtyfps_compilerlib::diagnostics::load_from_path(path) {
            Ok(s) => s,
            Err(d) => return (None, vec![d]),
        };

        // We create here a 'static guard. That's alright because we make sure
        // in this module that we only use erased component
        let guard = unsafe { generativity::Guard::new(generativity::Id::new()) };

        let (c, diag) =
            crate::dynamic_component::load(source, path.into(), config.config, guard).await;
        (c.ok().map(|inner| Self { inner }), diag.into_iter().collect())
    }

    /// Compile some .60 code into a ComponentDefinition
    ///
    /// The `path` argument will be used for diagnostics and to compute relative
    /// path while importing
    ///
    /// The first element of the returned tuple is going to be the compiled
    /// `ComponentDefinition` if there was no errors. This function also return
    /// a vector if diagnostics with errors and/or warnings.
    /// The [`print_diagnostics`] function can be used to display the diagnostics
    /// to the users.
    ///
    /// This function is `async` but in practice, this is only asynchronious if
    /// [`CompilerConfiguration::with_file_loader`] is set and its future is actually asynchronious.
    /// If that is not used, then it is fine to use a very simple executor, such as the one
    /// provided by the `spin_on` crate
    pub async fn from_source(
        source_code: String,
        path: PathBuf,
        config: CompilerConfiguration,
    ) -> (Option<Self>, Vec<Diagnostic>) {
        // We create here a 'static guard. That's alright because we make sure
        // in this module that we only use erased component
        let guard = unsafe { generativity::Guard::new(generativity::Id::new()) };

        let (c, diag) =
            crate::dynamic_component::load(source_code, path, config.config, guard).await;
        (c.ok().map(|inner| Self { inner }), diag.into_iter().collect())
    }

    /// Instantiate the component
    pub fn create(&self) -> ComponentInstance {
        ComponentInstance {
            inner: self.inner.clone().create(
                #[cfg(target_arch = "wasm32")]
                "canvas".into(),
            ),
        }
    }

    /// Instantiate the component for wasm using the given canvas id
    #[cfg(target_arch = "wasm32")]
    pub fn create_with_canvas_id(&self, canvas_id: &str) -> ComponentInstance {
        ComponentInstance { inner: self.inner.clone().create(canvas_id.into()) }
    }

    /// List of publicly declared properties or callback.
    ///
    /// This is internal because it exposes the `Type` from compilerlib.
    /// In the future this should probably return an iterator instead.
    #[doc(hidden)]
    pub fn properties(&self) -> HashMap<String, sixtyfps_compilerlib::langtype::Type> {
        self.inner.properties()
    }

    /// The name of this Component as written in the .60 file
    pub fn name(&self) -> &str {
        self.inner.id()
    }
}

/// Print the diagnostics to stderr
///
/// The diagnostics are printed in the same style as rustc errors
///
/// This function is available when the `display-diagnostics` is enabled.
#[cfg(feature = "display-diagnostics")]
pub fn print_diagnostics(diagnostics: &[Diagnostic]) {
    let mut build_diagnostics = sixtyfps_compilerlib::diagnostics::BuildDiagnostics::default();
    for d in diagnostics {
        build_diagnostics.push_compiler_error(d.clone())
    }
    build_diagnostics.print();
}

/// This represent an instance of a dynamic component
///
/// You can create an instance with the [`ComponentDefinition::create`] function.
///
/// Properties and callback can be accessed using the associated functions.
///
/// An instance can be put on screen with the [`ComponentInstance::run`] function.
pub struct ComponentInstance {
    inner: vtable::VRc<
        sixtyfps_corelib::component::ComponentVTable,
        crate::dynamic_component::ErasedComponentBox,
    >,
}

impl ComponentInstance {
    /// Return the [`ComponentDefinition`] that was used to create this instance.
    pub fn definition(&self) -> ComponentDefinition {
        // We create here a 'static guard. That's alright because we make sure
        // in this module that we only use erased component
        let guard = unsafe { generativity::Guard::new(generativity::Id::new()) };
        ComponentDefinition { inner: self.inner.unerase(guard).description() }
    }

    /// Return the value for a public property of this component.
    ///
    /// ## Examples
    ///
    /// ```
    /// use sixtyfps_interpreter::{ComponentDefinition, CompilerConfiguration, Value, SharedString};
    /// let code = r#"
    ///     MyWin := Window {
    ///         property <int> my_property: 42;
    ///     }
    /// "#;
    /// let (definition, diagnostics) = spin_on::spin_on(
    ///     ComponentDefinition::from_source(code.into(), Default::default(), CompilerConfiguration::new()));
    /// assert!(diagnostics.is_empty(), "{:?}", diagnostics);
    /// let instance = definition.unwrap().create();
    /// assert_eq!(instance.get_property("my_property").unwrap(), Value::from(42));
    /// ```
    pub fn get_property(&self, name: &str) -> Result<Value, GetPropertyError> {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        comp.description()
            .get_property(comp.borrow(), name)
            .map_err(|()| GetPropertyError::NoSuchProperty)
    }

    /// Set the value for a public property of this component
    pub fn set_property(&self, name: &str, value: Value) -> Result<(), SetPropertyError> {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        comp.description()
            .set_property(comp.borrow(), name, value)
            .map_err(|()| todo!("set_property don't return the right error type"))
    }

    /// Set a handler for the callback with the given name. A callback with that
    /// name must be defined in the document otherwise an error will be returned.
    ///
    /// Note: Since the [`ComponentInstance`] holds the handler, the handler itself should not
    /// contain a strong reference to the instance. So if you need to capture the instance,
    /// you should use [`Self::as_weak`] to create a weak reference.
    ///
    /// ## Examples
    ///
    /// ```
    /// use sixtyfps_interpreter::{ComponentDefinition, CompilerConfiguration, Value, SharedString};
    /// use core::convert::TryInto;
    /// let code = r#"
    ///     MyWin := Window {
    ///         callback foo(int) -> int;
    ///         property <int> my_prop: 12;
    ///     }
    /// "#;
    /// let (definition, _) = spin_on::spin_on(
    ///     ComponentDefinition::from_source(code.into(), Default::default(), CompilerConfiguration::new()));
    /// let instance = definition.unwrap().create();
    ///
    /// let instance_weak = instance.as_weak();
    /// instance.set_callback("foo", move |args: &[Value]| -> Value {
    ///     let arg: u32 = args[0].clone().try_into().unwrap();
    ///     let my_prop = instance_weak.unwrap().get_property("my_prop").unwrap();
    ///     let my_prop : u32 = my_prop.try_into().unwrap();
    ///     Value::from(arg + my_prop)
    /// }).unwrap();
    ///
    /// let res = instance.invoke_callback("foo", &[Value::from(500)]).unwrap();
    /// assert_eq!(res, Value::from(500+12));
    /// ```
    pub fn set_callback(
        &self,
        name: &str,
        callback: impl Fn(&[Value]) -> Value + 'static,
    ) -> Result<(), SetCallbackError> {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        comp.description()
            .set_callback_handler(comp.borrow(), name, Box::new(callback))
            .map_err(|()| SetCallbackError::NoSuchCallback)
    }

    /// Call the given callback with the arguments
    ///
    /// ## Examples
    /// See the documentation of [`Self::set_callback`] for an example
    pub fn invoke_callback(&self, name: &str, args: &[Value]) -> Result<Value, CallCallbackError> {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        Ok(comp.description().invoke_callback(comp.borrow(), name, &args).map_err(|()| todo!())?)
    }

    /// Marks the window of this component to be shown on the screen. This registers
    /// the window with the windowing system. In order to react to events from the windowing system,
    /// such as draw requests or mouse/touch input, it is still necessary to spin the event loop,
    /// using [`crate::run_event_loop`].
    pub fn show(&self) {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        comp.window().show();
    }

    /// Marks the window of this component to be hidden on the screen. This de-registers
    /// the window from the windowing system and it will not receive any further events.
    pub fn hide(&self) {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        comp.window().hide();
    }
    /// This is a convenience function that first calls [`Self::show`], followed by [`crate::run_event_loop()`]
    /// and [`Self::hide`].
    pub fn run(&self) {
        self.show();
        sixtyfps_rendering_backend_default::backend().run_event_loop();
        self.hide();
    }

    /// Clone this `ComponentInstance`.
    ///
    /// A `ComponentInstance` is in fact a handle to a reference counted instance.
    /// This function is semanticallt the same as the one from `Clone::clone`, but
    /// Clone is not implemented because of the danger of circular reference:
    /// If you want to use this instance in a callback, you should capture a weak
    /// reference given by [`Self::as_weak`].
    pub fn clone_strong(&self) -> Self {
        Self { inner: self.inner.clone() }
    }

    /// Create a weak pointer to this component
    pub fn as_weak(&self) -> WeakComponentInstance {
        WeakComponentInstance { inner: vtable::VRc::downgrade(&self.inner) }
    }
}

/// A Weak references to a dynamic SixtyFPS components.
#[derive(Clone)]
pub struct WeakComponentInstance {
    inner: vtable::VWeak<
        sixtyfps_corelib::component::ComponentVTable,
        crate::dynamic_component::ErasedComponentBox,
    >,
}

impl WeakComponentInstance {
    /// Returns a new strongly referenced component if some other instance still
    /// holds a strong reference. Otherwise, returns None.
    pub fn upgrade(&self) -> Option<ComponentInstance> {
        self.inner.upgrade().map(|inner| ComponentInstance { inner })
    }

    /// Convenience function that returns a new stronlyg referenced component if
    /// some other instance still holds a strong reference. Otherwise, this function
    /// panics.
    pub fn unwrap(&self) -> ComponentInstance {
        self.upgrade().unwrap()
    }
}

/// Error returned by [`ComponentInstance::get_property`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GetPropertyError {
    /// There is no property with the given name
    #[error("no such property")]
    NoSuchProperty,
}

/// Error returned by [`ComponentInstance::set_property`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SetPropertyError {
    /// There is no property with the given name
    #[error("no such property")]
    NoSuchProperty,
    /// The property exist but does not have a type matching the dynamic value
    #[error("wrong type")]
    WrongType,
}

/// Error returned by [`ComponentInstance::set_callback`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SetCallbackError {
    /// There is no callback with the given name
    #[error("no such callback")]
    NoSuchCallback,
}

/// Error returned by [`ComponentInstance::invoke_callback`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CallCallbackError {
    /// There is no callback with the given name
    #[error("no such callback")]
    NoSuchCallback,
}

/// The structure for configuring aspects of the compilation of `.60` markup files to Rust.
pub struct CompilerConfiguration {
    config: sixtyfps_compilerlib::CompilerConfiguration,
}

impl CompilerConfiguration {
    /// Creates a new default configuration.
    pub fn new() -> Self {
        Self {
            config: sixtyfps_compilerlib::CompilerConfiguration::new(
                sixtyfps_compilerlib::generator::OutputFormat::Interpreter,
            ),
        }
    }

    /// Create a new configuration that includes sets the include paths used for looking up
    /// `.60` imports to the specified vector of paths.
    pub fn with_include_paths(self, include_paths: Vec<std::path::PathBuf>) -> Self {
        let mut config = self.config;
        config.include_paths = include_paths;
        Self { config }
    }

    /// Create a new configuration that selects the style to be used for widgets.
    pub fn with_style(self, style: String) -> Self {
        let mut config = self.config;
        config.style = Some(style);
        Self { config }
    }

    /// Create a new configuration that will use the provided callback for loading.
    pub fn with_file_loader(
        self,
        file_loader_fallback: impl Fn(
                &Path,
            )
                -> core::pin::Pin<Box<dyn core::future::Future<Output = std::io::Result<String>>>>
            + 'static,
    ) -> Self {
        let mut config = self.config;
        config.open_import_fallback =
            Some(Box::new(move |path| file_loader_fallback(Path::new(path.as_str()))));
        Self { config }
    }
}

/// Enters the main event loop. This is necessary in order to receive
/// events from the windowing system in order to render to the screen
/// and react to user input.
pub fn run_event_loop() {
    sixtyfps_rendering_backend_default::backend().run_event_loop();
}

/// This module constains a few function use by tests
pub mod testing {
    /// Wrapper around [`sixtyfps_corelib::tests::sixtyfps_send_mouse_click`]
    pub fn send_mouse_click(comp: &super::ComponentInstance, x: f32, y: f32) {
        sixtyfps_corelib::tests::sixtyfps_send_mouse_click(
            &vtable::VRc::into_dyn(comp.inner.clone()),
            x,
            y,
            &comp.inner.window(),
        );
    }
    /// Wrapper around [`sixtyfps_corelib::tests::send_keyboard_string_sequence`]
    pub fn send_keyboard_string_sequence(
        comp: &super::ComponentInstance,
        string: sixtyfps_corelib::SharedString,
    ) {
        sixtyfps_corelib::tests::send_keyboard_string_sequence(
            &string,
            Default::default(),
            &comp.inner.window(),
        );
    }
}

/// For the C++ integration
#[cfg(feature = "ffi")]
pub mod ffi {
    use super::*;

    /// This is casted to a Value
    #[repr(C)]
    pub struct ValueOpaque([usize; 7]);

    /// Asserts that ValueOpaque is at least as large as Value, otherwise this would overflow
    const _: usize = std::mem::size_of::<ValueOpaque>() - std::mem::size_of::<Value>();

    /// Construct a new Value in the given memory location
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_interpreter_value_new(val: *mut ValueOpaque) {
        std::ptr::write(val as *mut Value, Value::default())
    }

    /// Destruct the value in that memory location
    #[no_mangle]
    pub unsafe extern "C" fn sixtyfps_interpreter_value_destructor(val: *mut ValueOpaque) {
        drop(std::ptr::read(val as *mut Value))
    }
}
