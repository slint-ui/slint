/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#![warn(missing_docs)]

use core::convert::TryInto;
use std::collections::HashMap;
use std::iter::FromIterator;
use std::path::Path;
use std::rc::Rc;

use crate::eval;

#[doc(inline)]
pub use sixtyfps_compilerlib::diagnostics::{Diagnostic, DiagnosticLevel};

/// This is a dynamically typed value used in the SixtyFPS interpreter.
/// It can hold a value of different types, and you should use the
/// [`From`] or [`TryInto`] traits to access the value.
///
/// ```
/// # use sixtyfps_interpreter::api::*;
/// use core::convert::TryInto;
/// // create a value containing an integer
/// let v = Value::from(100u32);
/// assert_eq!(v.try_into(), Ok(100u32));
/// ```
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Value(pub(crate) eval::Value);

/// A dummy structure that can be converted to and from [`Value`].
///
/// A default constructed Value holds this value.
/// ```
/// # use sixtyfps_interpreter::api::*;
/// let value = Value::default();
/// assert_eq!(Value::default(), VoidValue.into());
/// ```

#[derive(Clone, PartialEq, Debug, Default)]
pub struct VoidValue;

impl From<VoidValue> for Value {
    fn from(_: VoidValue) -> Self {
        Self(eval::Value::Void)
    }
}
impl TryInto<VoidValue> for Value {
    type Error = Value;
    fn try_into(self) -> Result<VoidValue, Value> {
        if self.0 == eval::Value::Void {
            Ok(VoidValue)
        } else {
            Err(self)
        }
    }
}

macro_rules! pub_value_conversion {
    ($($ty:ty,)*) => {
        $(
            impl From<$ty> for Value {
                fn from(v: $ty) -> Self {
                    Self(v.try_into().unwrap())
                }
            }
            impl TryInto<$ty> for Value {
                type Error = Value;
                fn try_into(self) -> Result<$ty, Value> {
                    self.0.try_into().map_err(Value)
                }
            }
        )*
    };
}

pub_value_conversion!(
    f32,
    f64,
    i32,
    u32,
    bool,
    sixtyfps_corelib::SharedString,
    sixtyfps_corelib::Color,
    //sixtyfps_corelib::Brush,
);

// TODO! model

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
/// # use sixtyfps_interpreter::api::*;
/// use core::convert::TryInto;
/// // Construct a value from a key/value iterator
/// let value : Value = [("foo".into(), 45u32.into()), ("bar".into(), true.into())]
///     .iter().cloned().collect::<Struct>().into();
///
/// // get the properties of a `{ foo: 45, bar: true }`
/// let s : Struct = value.try_into().unwrap();
/// assert_eq!(s.get_property("foo").unwrap().try_into(), Ok(45u32));
/// ```
/// FIXME: the documentation of langref.md uses "Object" and we probably should make that uniform.
///        also, is "property" the right term here?
pub struct Struct(HashMap<String, eval::Value>);
impl Struct {
    /// Get the value for a given struct property
    pub fn get_property(&self, name: &str) -> Option<Value> {
        self.0.get(name).cloned().map(Value)
    }
    /// Set the value of a given struct property
    pub fn set_property(&mut self, name: String, value: Value) {
        self.0.insert(name, value.0);
    }

    /// Iterate over all the property in this struct
    pub fn iter(&self) -> impl Iterator<Item = (&str, Value)> {
        self.0.iter().map(|(a, b)| (a.as_str(), Value(b.clone())))
    }
}

impl From<Struct> for Value {
    fn from(s: Struct) -> Self {
        Self(eval::Value::Object(s.0))
    }
}
impl TryInto<Struct> for Value {
    type Error = Value;
    fn try_into(self) -> Result<Struct, Value> {
        if let eval::Value::Object(o) = self.0 {
            Ok(Struct(o))
        } else {
            Err(self)
        }
    }
}

impl FromIterator<(String, Value)> for Struct {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        Self(iter.into_iter().map(|(a, b)| (a, b.0)).collect())
    }
}

/// FIXME: use SharedArray instead?
impl From<Vec<Value>> for Value {
    fn from(a: Vec<Value>) -> Self {
        Self(eval::Value::Array(a.into_iter().map(|v| v.0).collect()))
    }
}
impl TryInto<Vec<Value>> for Value {
    type Error = Value;
    fn try_into(self) -> Result<Vec<Value>, Value> {
        if let eval::Value::Array(a) = self.0 {
            Ok(a.into_iter().map(Value).collect())
        } else {
            Err(self)
        }
    }
}

/// ComponentDescription is a representation of a compiled component from .60
///
/// It can be constructed from a .60 file using the [`Self::from_path`] or [`Self::from_string`] functions.
/// And then it can be instentiated with the [`Self::create`] function
#[derive(Clone)]
pub struct ComponentDefinition {
    inner: Rc<crate::dynamic_component::ComponentDescription<'static>>,
}

impl ComponentDefinition {
    /// Compile a .60 file into a ComponentDefinition
    ///
    /// The first element of the returned tuple is going to be the compiled
    /// ComponentDefinition if there was no errors. This function also return
    /// a vector if diagnostics with errors and/or warnings
    pub async fn from_path<P: AsRef<Path>>(
        path: P,
        config: CompilerConfiguration,
    ) -> (Option<Self>, Vec<Diagnostic>) {
        let path = path.as_ref();
        let source = match sixtyfps_compilerlib::diagnostics::load_from_path(path) {
            Ok(s) => s,
            Err(d) => return (None, vec![d]),
        };

        let (c, diag) = crate::load(source, path.into(), config.config).await;
        (c.ok().map(|inner| Self { inner }), diag.into_iter().collect())
    }
    /// Compile some .60 code into a ComponentDefinition
    ///
    /// The first element of the returned tuple is going to be the compiled
    /// ComponentDefinition if there was no errors. This function also return
    /// a vector if diagnostics with errors and/or warnings
    pub async fn from_string(
        source_code: &str,
        config: CompilerConfiguration,
    ) -> (Option<Self>, Vec<Diagnostic>) {
        let (c, diag) = crate::load(source_code.into(), Default::default(), config.config).await;
        (c.ok().map(|inner| Self { inner }), diag.into_iter().collect())
    }

    /// Instantiate the component
    ///
    /// FIXME! wasm canvas id?
    pub fn create(&self) -> ComponentInstance {
        ComponentInstance {
            inner: self.inner.clone().create(
                #[cfg(target_arch = "wasm32")]
                todo!(),
            ),
        }
    }

    /// List of publicly declared properties or callback.
    /// FIXME: this expose `sixtyfps_compilerlib::Type`  (should it perhaps return an iterator instead?)
    pub fn properties(&self) -> HashMap<String, sixtyfps_compilerlib::langtype::Type> {
        self.inner.properties()
    }

    /// The name of this Component as written in the .60 file
    pub fn id(&self) -> &str {
        self.inner.id()
    }
}

/// This represent an instance of a dynamic component
/// FIXME: Clone?  (same problem as for the generated component that can be cloned but maybe not such a good idea)
#[derive(Clone)]
pub struct ComponentInstance {
    inner: vtable::VRc<
        sixtyfps_corelib::component::ComponentVTable,
        crate::dynamic_component::ErasedComponentBox,
    >,
}

impl ComponentInstance {
    /// Return the value for a public property of this component
    pub fn get_property(&self, name: &str) -> Result<Value, NoSuchPropertyError> {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        comp.description()
            .get_property(comp.borrow(), name)
            .map(|v| Value(v))
            .map_err(|()| NoSuchPropertyError)
    }

    /// Return the value for a public property of this component
    pub fn set_property(&self, name: &str, value: Value) -> Result<(), SetPropertyError> {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        comp.description()
            .set_property(comp.borrow(), name, value.0)
            .map_err(|()| todo!("set_property don't return the right error type"))
    }

    /// FIXME: error type.
    /// FIXME: what to do if the returned value is not the right type (currently! panic in the eval)
    /// FIXME: name: should it be called on_callback?
    pub fn set_callback_handler(
        &self,
        name: &str,
        callback: impl Fn(&[Value]) -> Value + 'static,
    ) -> Result<(), NoSuchPropertyError> {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        comp.description()
            .set_callback_handler(
                comp.borrow(),
                name,
                Box::new(move |args| {
                    // TODO: avoid allocation in common case by puting that on the stack or find a way (transmute?)
                    let args = args.iter().map(|v| Value(v.clone())).collect::<Vec<_>>();
                    callback(&args).0
                }),
            )
            .map_err(|()| NoSuchPropertyError)
    }

    /// Call the given callback with the arguments
    pub fn call_callback(&self, name: &str, args: &[Value]) -> Result<Value, CallCallbackError> {
        generativity::make_guard!(guard);
        let comp = self.inner.unerase(guard);
        // TODO: avoid allocation in common case by puting that on the stack or find a way (transmute?)
        let args = args.iter().map(|v| v.0.clone()).collect::<Vec<_>>();
        Ok(Value(
            comp.description().call_callback(comp.borrow(), name, &args).map_err(|()| todo!())?,
        ))
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
        crate::run_event_loop();
        self.hide();
    }
}

/// Error returned by [`ComponentInstance::get_property`] if the component does not have that property
#[derive(Debug)]
pub struct NoSuchPropertyError;

/// Error returned by [`ComponentInstance::set_property`]
#[derive(Debug)]
pub enum SetPropertyError {
    /// There is no property with the given name
    NoSuchProperty,
    /// The property exist but does not have a type matching the dynamic value
    WrongType,
}

/// Error returned by [`ComponentInstance::call_callback`]
pub enum CallCallbackError {
    //TODO
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
}
