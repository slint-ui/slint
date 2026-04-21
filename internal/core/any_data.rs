// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Dynamically-typed data, for runtime-generic code such as [`ClipboardData`](crate::clipboard::ClipboardData).

use core::any::Any;

#[cfg(feature = "path")]
use crate::PathData;

use crate::{
    Brush, Color, SharedString, SharedVector,
    animations::EasingCurve,
    api::{Image, Keys, LogicalPosition},
    component_factory::ComponentFactory,
    graphics::Point,
    items::*,
    lengths::{LogicalEdges, LogicalLength, LogicalPoint, LogicalSize, PhysicalEdges},
    model::ModelRc,
    styled_text::StyledText,
};
use alloc::boxed::Box;

/// A piece of data of unspecified type. Use the accessor methods to downcast this to a specific type.
#[derive(Default, PartialEq, Clone)]
pub struct AnyData {
    inner: AnyDataInner,
}

pub trait AnyValue: Any {
    fn dyn_eq(&self, other: &dyn AnyValue) -> bool;
    fn dyn_clone(&self) -> Box<dyn AnyValue>;
}

impl<T> AnyValue for T
where
    T: Any + PartialEq + Clone,
{
    fn dyn_eq(&self, other: &dyn AnyValue) -> bool {
        (other as &dyn Any).downcast_ref::<Self>().is_some_and(|other| other == self)
    }

    fn dyn_clone(&self) -> Box<dyn AnyValue> {
        Box::new(self.clone())
    }
}

impl PartialEq for dyn AnyValue {
    fn eq(&self, other: &Self) -> bool {
        AnyValue::dyn_eq(self, other)
    }
}

impl Clone for Box<dyn AnyValue> {
    fn clone(&self) -> Self {
        self.dyn_clone()
    }
}

impl dyn AnyValue {
    fn downcast<T: Any>(self: Box<Self>) -> Result<T, Box<Self>> {
        if (&*self as &dyn Any).is::<T>() {
            Ok(*(self as Box<dyn Any>).downcast::<T>().unwrap())
        } else {
            Err(self)
        }
    }
}

#[derive(Default, PartialEq, Clone)]
enum AnyDataInner {
    #[default]
    Void,
    Number(f64),
    String(SharedString),
    Bool(bool),
    Image(Image),
    EasingCurve(EasingCurve),
    StyledText(StyledText),
    Any(Box<dyn AnyValue>),
}

macro_rules! declare_anydata_any {
    ( $($ty:ty),* ) => {
        $(
            impl From<$ty> for AnyData{
                fn from(v: $ty) -> Self {
                    AnyData { inner: AnyDataInner::Any(Box::new(v)) }
                }
            }

            impl TryFrom<AnyData> for $ty {
                type Error = AnyData;

                fn try_from(v: AnyData) -> Result<Self, Self::Error> {
                    match v {
                        AnyData { inner: AnyDataInner::Any(inner) }
                            if let Some(v) = (&*inner as &dyn Any).downcast_ref::<$ty>().cloned() =>
                            Ok(v),
                        _ => Err(v),
                    }
                }
            }
        )*
    }
}

macro_rules! declare_anydata_struct {
    (struct $name:path { $($field:ident),* $(, ..$extra:expr)? }) => {
        declare_anydata_any!($name);
    };
    ($(
        $(#[$struct_attr:meta])*
        struct $Name:ident {
            @name = $inner_name:expr,
            export {
                $( $(#[$pub_attr:meta])* $pub_field:ident : $pub_type:ty, )*
            }
            private { $($pri:tt)* }
        }
    )*) => {
        declare_anydata_any!($($Name),*);
    };
}

macro_rules! declare_anydata_enum {
    ($( $(#[$enum_doc:meta])* enum $Name:ident { $($body:tt)* })*) => {
        declare_anydata_any!($($Name),*);
    };
}

macro_rules! declare_anydata_conversion {
    ( $value:ident => [$($ty:ty),*] ) => {
        $(
            impl From<$ty> for AnyData{
                fn from(v: $ty) -> Self {
                    AnyData { inner: AnyDataInner::$value(v as _) }
                }
            }

            impl TryFrom<AnyData> for $ty {
                type Error = AnyData;
                fn try_from(v: AnyData) -> Result<$ty, Self::Error> {
                    match v {
                        AnyData{ inner: AnyDataInner::$value(x) } => Ok(x as _),
                        _ => Err(v)
                    }
                }
            }
        )*
    };
}

impl From<()> for AnyData {
    fn from((): ()) -> Self {
        AnyData { inner: AnyDataInner::Void }
    }
}

impl TryFrom<AnyData> for () {
    type Error = AnyData;

    fn try_from(value: AnyData) -> Result<Self, Self::Error> {
        match value {
            AnyData { inner: AnyDataInner::Void } => Ok(()),
            _ => Err(value),
        }
    }
}

declare_anydata_conversion!(Number => [u32, u64, i32, i64, f32, f64, usize, isize] );
declare_anydata_conversion!(String => [SharedString] );
declare_anydata_conversion!(Bool => [bool] );
declare_anydata_conversion!(Image => [Image] );
declare_anydata_any!(
    Brush,
    LogicalPosition,
    LogicalPoint,
    LogicalEdges,
    LogicalSize,
    LogicalLength,
    PhysicalEdges,
    Point,
    Color,
    SharedVector<u16>
);
#[cfg(feature = "path")]
declare_anydata_any!(PathData);
declare_anydata_conversion!(EasingCurve => [EasingCurve]);
declare_anydata_any!(SharedVector<f32>);
declare_anydata_any!(ComponentFactory);
declare_anydata_conversion!(StyledText => [StyledText] );
declare_anydata_any!(Keys);
i_slint_common::for_each_builtin_structs!(declare_anydata_struct);
i_slint_common::for_each_enums!(declare_anydata_enum);

impl<T: Into<AnyData> + TryFrom<AnyData> + 'static> From<ModelRc<T>> for AnyData {
    fn from(v: ModelRc<T>) -> Self {
        AnyData { inner: AnyDataInner::Any(Box::new(v)) }
    }
}

impl<T: TryFrom<AnyData> + Default + 'static> TryFrom<AnyData> for ModelRc<T> {
    type Error = AnyData;

    #[inline]
    fn try_from(v: AnyData) -> Result<ModelRc<T>, Self::Error> {
        match v {
            AnyData { inner: AnyDataInner::Any(inner) }
                if let Some(v) = (&*inner as &dyn Any).downcast_ref::<Self>().cloned() =>
            {
                Ok(v)
            }
            _ => Err(v),
        }
    }
}

impl AnyData {
    /// Returns a reference to the inner value if it is of type `T`, or `None` if it isn’t.
    pub fn as_string(&self) -> Option<SharedString> {
        if let AnyDataInner::String(string) = &self.inner { Some(string.clone()) } else { None }
    }

    pub fn downcast<T: Any>(self) -> Result<T, Self> {
        let value = match self.inner {
            AnyDataInner::Any(any_value) => any_value,
            AnyDataInner::Void => Box::new(()) as Box<dyn AnyValue>,
            AnyDataInner::Number(val) => Box::new(val) as Box<dyn AnyValue>,
            AnyDataInner::String(val) => Box::new(val) as Box<dyn AnyValue>,
            AnyDataInner::Bool(val) => Box::new(val) as Box<dyn AnyValue>,
            AnyDataInner::Image(val) => Box::new(val) as Box<dyn AnyValue>,
            AnyDataInner::EasingCurve(val) => Box::new(val) as Box<dyn AnyValue>,
            AnyDataInner::StyledText(val) => Box::new(val) as Box<dyn AnyValue>,
        };

        value.downcast().map_err(|value| AnyData { inner: AnyDataInner::Any(value) })
    }

    pub fn from_any<T: Any + Clone + PartialEq>(value: T) -> Self {
        if (&value as &dyn Any).is::<()>() {
            return Self { inner: AnyDataInner::Void };
        }

        // Hopefully this will be omitted by the compiler.
        let mut value = Some(Box::new(value) as Box<dyn AnyValue>);

        macro_rules! check_type_and_ret {
            ($typ:ty, $variant:ident) => {
                match value.take().unwrap().downcast::<$typ>() {
                    Ok(value) => return Self { inner: AnyDataInner::$variant(value) },
                    Err(other) => value = Some(other),
                }
            };
        }

        check_type_and_ret!(f64, Number);
        check_type_and_ret!(SharedString, String);
        check_type_and_ret!(bool, Bool);
        check_type_and_ret!(Image, Image);
        check_type_and_ret!(EasingCurve, EasingCurve);
        check_type_and_ret!(StyledText, StyledText);

        Self { inner: AnyDataInner::Any(value.take().unwrap()) }
    }
}
