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
use std::todo;

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
pub struct Value(pub(crate) crate::eval::Value);

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
        Self(crate::eval::Value::Void)
    }
}
impl TryInto<VoidValue> for Value {
    /// FIXME: better error?
    type Error = ();
    fn try_into(self) -> Result<VoidValue, ()> {
        if self.0 == crate::eval::Value::Void {
            Ok(VoidValue)
        } else {
            Err(())
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
                /// FIXME: better error?
                type Error = ();
                fn try_into(self) -> Result<$ty, ()> {
                    self.0.try_into()
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
pub struct Struct(HashMap<String, crate::eval::Value>);
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
        Self(crate::eval::Value::Object(s.0))
    }
}
impl TryInto<Struct> for Value {
    /// FIXME: better error?
    type Error = ();
    fn try_into(self) -> Result<Struct, ()> {
        if let crate::eval::Value::Object(o) = self.0 {
            Ok(Struct(o))
        } else {
            Err(())
        }
    }
}

impl FromIterator<(String, Value)> for Struct {
    fn from_iter<T: IntoIterator<Item = (String, Value)>>(iter: T) -> Self {
        Self(iter.into_iter().map(|(a, b)| (a, b.0)).collect())
    }
}

/// ComponentDescription is a representation of a compiled component from .60
///
/// It can be constructed from a .60 file using the [`Self::from_path`] or [`Self::from_string`] functions.
/// And then it can be instentiated with the [`Self::create`] function
pub struct ComponentDefinition {
    inner: Rc<crate::dynamic_component::ComponentDescription<'static>>,
}

impl ComponentDefinition {
    /// Compile a .60 file into a ComponentDefinition
    pub async fn from_path<P: AsRef<Path>>(
        path: P,
    ) -> Result<ComponentDefinition, ComponentLoadError> {
        let inner = crate::load(
            std::fs::read_to_string(&path).map_err(|_| todo!())?,
            path.as_ref().into(),
            crate::new_compiler_configuration(),
        )
        .await
        .0
        .map_err(|_| todo!())?;
        Ok(Self { inner })
    }
    /// Compile some .60 code into a ComponentDefinition
    pub async fn from_string(source_code: &str) -> Result<ComponentDefinition, ComponentLoadError> {
        let inner = crate::load(
            source_code.into(),
            Default::default(),
            crate::new_compiler_configuration(),
        )
        .await
        .0
        .map_err(|_| todo!())?;
        Ok(Self { inner })
    }

    /// Instantiate the component
    pub fn create(&self) -> ComponentInstance {
        ComponentInstance {
            inner: self.inner.clone().create(
                #[cfg(target_arch = "wasm32")]
                todo!(),
            ),
        }
    }
}

/// Error returned if constructing a [`ComponentDefinition`] fails
pub enum ComponentLoadError {
    // TODO
}

/// This represent an instance of a dynamic component
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
    /// FIXME: name: should it be called set_callback_handler?
    pub fn on_callback(
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
    //todo
}
