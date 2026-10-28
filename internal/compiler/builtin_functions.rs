// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The declaration of the functions built into the run-time

use crate::expression_types::Orientation;
use crate::langtype::Function;
use smol_str::{SmolStr, StrExt};
use std::sync::Arc;

macro_rules! declare_builtin_functions {
    ($(
        $(#[$attr:meta])*
        $Name:ident $(($Payload:ident))? : $(#[$runtime_attr:meta])*
            $purity:ident fn $($runtime:ident)?
                ($(& $context:ident $(,)?)? $($param:ident: $ty:tt $(<$gen:ident>)? $(=> $as:tt)?),*)
                -> $Ret:tt $(=> $RetType:tt)? $($body:block)?
    )*) => {
        /// A function built into the run-time.
        /// The functions of builtin elements name their variant in `builtin_elements.rs`.
        #[derive(Debug, Clone, PartialEq, Eq, strum::EnumString)]
        #[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
        pub enum BuiltinFunction {
            $($(#[$attr])* $Name $(($Payload))?,)*
        }

        /// A pattern that matches the builtins that have a `runtime` function.
        /// Generators match the others explicitly, so a new builtin without one fails to compile.
        #[macro_export]
        macro_rules! runtime_builtin_function {
            () => { $($(| $crate::first!($crate::builtin_functions::BuiltinFunction::$Name, $runtime))?)* };
        }

        #[allow(non_snake_case)]
        struct BuiltinFunctionTypes {
            $($Name: Arc<Function>,)*
        }

        impl BuiltinFunction {
            pub fn ty(&self) -> Arc<Function> {
                static TYPES: std::sync::LazyLock<BuiltinFunctionTypes> =
                    std::sync::LazyLock::new(|| BuiltinFunctionTypes {
                        $($Name: Arc::new(Function {
                            args: vec![$(builtin_type!($ty $(=> $as)?)),*],
                            return_type: builtin_type!($Ret $(=> $RetType)?),
                            arg_names: arg_names(&[$(stringify!($param)),*]),
                        }),)*
                    });
                match self {
                    $(Self::$Name { .. } => TYPES.$Name.clone(),)*
                }
            }


            /// See `pure` in [`i_slint_common::for_each_builtin_function`]
            pub fn is_pure(&self) -> bool {
                match self {
                    $(Self::$Name { .. } => builtin_purity!($purity),)*
                }
            }

            /// Whether the `runtime` function takes the component's context first,
            /// see [`i_slint_common::for_each_builtin_function`]
            pub fn runtime_function_takes_context(&self) -> bool {
                match self {
                    $(Self::$Name { .. } => false $(|| first!(true, $context))?,)*
                }
            }

            /// See `runtime` in [`i_slint_common::for_each_builtin_function`]
            pub fn runtime_function(&self) -> Option<&'static str> {
                match self {
                    $(Self::$Name { .. } => [$(stringify!($runtime))?].first().copied(),)*
                }
            }
        }
    };
}

fn arg_names(names: &[&str]) -> Vec<SmolStr> {
    names.iter().map(|name| name.replace_smolstr("_", "-")).collect()
}

/// Expands to its first argument, for a repetition driven by the second.
#[doc(hidden)]
#[macro_export]
macro_rules! first {
    ($first:literal, $_:tt) => {
        $first
    };
    ($first:path, $_:tt) => {
        $first
    };
}

macro_rules! builtin_purity {
    (pure) => {
        true
    };
    (impure) => {
        false
    };
}

/// The `Type` of a type in [`i_slint_common::for_each_builtin_function`] or
/// `i_slint_common::for_each_builtin_structs`: a Rust type, `RustType => SlintType`,
/// or the name of a Slint type or enum.
#[rustfmt::skip]
macro_rules! builtin_type {
    ($rust:tt => $slint:tt) => { builtin_type!($slint) };
    (()) => { $crate::langtype::Type::Void };
    ([$ty:tt]) => { $crate::langtype::Type::Array(std::sync::Arc::new(builtin_type!($ty))) };
    (f32) => { $crate::langtype::Type::Float32 };
    (f64) => { $crate::langtype::Type::Float32 };
    (i32) => { $crate::langtype::Type::Int32 };
    (bool) => { $crate::langtype::Type::Bool };
    (SharedString) => { $crate::langtype::Type::String };
    (Coord) => { $crate::langtype::Type::LogicalLength };
    (LogicalPosition) => { $crate::langtype::Type::Struct($crate::typeregister::logical_point_type()) };
    (FontMetrics) => { $crate::typeregister::font_metrics_type() };
    (LayoutInfo) => { $crate::typeregister::layout_info_type().into() };
    (RgbaColorU8) => { $crate::typeregister::builtin_structs::get(&$crate::langtype::BuiltinStruct::ColorRgba).into() };
    (HsvaColor) => { $crate::typeregister::builtin_structs::get(&$crate::langtype::BuiltinStruct::ColorHsva).into() };
    (OklchColor) => { $crate::typeregister::builtin_structs::get(&$crate::langtype::BuiltinStruct::ColorOklch).into() };
    (Size) => { $crate::typeregister::builtin_structs::get(&$crate::langtype::BuiltinStruct::Size).into() };
    (NoArgCallback) => { $crate::typeregister::noarg_callback_type() };
    (ScaleFactor) => {
        $crate::langtype::Type::UnitProduct(vec![
            ($crate::expression_types::Unit::Phx, 1),
            ($crate::expression_types::Unit::Px, -1),
        ])
    };
    (Int32) => { $crate::langtype::Type::Int32 };
    (Angle) => { $crate::langtype::Type::Angle };
    (Duration) => { $crate::langtype::Type::Duration };
    (ElementReference) => { $crate::langtype::Type::ElementReference };
    (Model) => { $crate::langtype::Type::Model };
    (Closure) => { $crate::langtype::Type::Closure };
    (InferredProperty) => { $crate::langtype::Type::InferredProperty };
    (Image) => { $crate::langtype::Type::Image };
    (Keys) => { $crate::langtype::Type::Keys };
    (DataTransfer) => { $crate::langtype::Type::DataTransfer };
    (Color) => { $crate::langtype::Type::Color };
    (Brush) => { $crate::langtype::Type::Brush };
    (StyledText) => { $crate::langtype::Type::StyledText };
    ($enum:ident) => { $crate::langtype::Type::Enumeration($crate::typeregister::BUILTIN.enums.$enum.clone()) };
}

i_slint_common::for_each_builtin_function!(declare_builtin_functions);
