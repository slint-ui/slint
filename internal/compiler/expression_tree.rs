// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::{BuildDiagnostics, SourceLocation, Spanned};
use crate::langtype::{BuiltinElement, EnumerationValue, Function, Struct, Type};
use crate::layout::Orientation;
use crate::lookup::LookupCtx;
use crate::object_tree::*;
use crate::parser::{NodeOrToken, SyntaxNode};
use crate::typeregister;
use core::cell::RefCell;
use smol_str::{format_smolstr, SmolStr};
use std::cell::Cell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

// FIXME remove the pub
pub use crate::namedreference::NamedReference;
pub use crate::passes::resolving;

#[derive(Debug, Clone, PartialEq, Eq)]
/// A function built into the run-time
pub enum BuiltinFunction {
    GetWindowScaleFactor,
    GetWindowDefaultFontSize,
    AnimationTick,
    Debug,
    Mod,
    Round,
    Ceil,
    Floor,
    Abs,
    Sqrt,
    Cos,
    Sin,
    Tan,
    ACos,
    ASin,
    ATan,
    ATan2,
    Log,
    Ln,
    Pow,
    Exp,
    ToFixed,
    ToPrecision,
    SetFocusItem,
    ClearFocusItem,
    ShowPopupWindow,
    ClosePopupWindow,
    /// Show a context popup menu.
    /// Arguments are `(parent, entries, position)`
    ///
    /// The first argument (parent) is a reference to the `ContectMenu` native item
    /// The second argument (entries) can either be of type Array of MenuEntry, or a reference to a MenuItem tree.
    /// When it is a menu item tree, it is a ElementReference to the root of the tree, and in the LLR, a NumberLiteral to an index in  [`crate::llr::SubComponent::menu_item_trees`]
    ShowPopupMenu,
    SetSelectionOffsets,
    ItemFontMetrics,
    /// the "42".to_float()
    StringToFloat,
    /// the "42".is_float()
    StringIsFloat,
    /// the "42".is_empty
    StringIsEmpty,
    /// the "42".length
    StringCharacterCount,
    StringToLowercase,
    StringToUppercase,
    ColorRgbaStruct,
    ColorHsvaStruct,
    ColorBrighter,
    ColorDarker,
    ColorTransparentize,
    ColorMix,
    ColorWithAlpha,
    ImageSize,
    ArrayLength,
    Rgb,
    Hsv,
    ColorScheme,
    SupportsNativeMenuBar,
    /// Setup the native menu bar, or the item-tree based menu bar
    /// arguments are: `(ref entries, ref sub-menu, ref activated, item_tree_root?, no_native_menu_bar?)`
    /// The two last arguments are only set if the menu is an item tree, in which case, `item_tree_root` is a reference
    /// to the MenuItem tree root (just like the entries in the [`Self::ShowPopupMenu`] call), and `native_menu_bar` is
    /// is a boolean literal that is true when we shouldn't try to setup the native menu bar.
    /// If we have an item_tree_root, the code will assign the callback handler and properties on the non-native menubar as well
    SetupNativeMenuBar,
    Use24HourFormat,
    MonthDayCount,
    MonthOffset,
    FormatDate,
    DateNow,
    ValidDate,
    ParseDate,
    TextInputFocused,
    SetTextInputFocused,
    ImplicitLayoutInfo(Orientation),
    ItemAbsolutePosition,
    RegisterCustomFontByPath,
    RegisterCustomFontByMemory,
    RegisterBitmapFont,
    Translate,
    UpdateTimers,
    DetectOperatingSystem,
    StartTimer,
    StopTimer,
    RestartTimer,
}

#[derive(Debug, Clone)]
/// A builtin function which is handled by the compiler pass
///
/// Builtin function expect their arguments in one and a specific type, so that's easier
/// for the generator. Macro however can do some transformation on their argument.
///
pub enum BuiltinMacroFunction {
    /// Transform `min(a, b, c, ..., z)` into a series of conditional expression and comparisons
    Min,
    /// Transform `max(a, b, c, ..., z)` into  a series of conditional expression and comparisons
    Max,
    /// Transforms `clamp(v, min, max)` into a series of min/max calls
    Clamp,
    /// Add the right conversion operations so that the return type is the same as the argument type
    Mod,
    /// Add the right conversion operations so that the return type is the same as the argument type
    Abs,
    CubicBezier,
    /// The argument can be r,g,b,a or r,g,b and they can be percentages or integer.
    /// transform the argument so it is always rgb(r, g, b, a) with r, g, b between 0 and 255.
    Rgb,
    Hsv,
    /// transform `debug(a, b, c)` into debug `a + " " + b + " " + c`
    Debug,
}

macro_rules! declare_builtin_function_types {
    ($( $Name:ident $(($Pattern:tt))? : ($( $Arg:expr ),*) -> $ReturnType:expr $(,)? )*) => {
        #[allow(non_snake_case)]
        pub struct BuiltinFunctionTypes {
            $(pub $Name : Rc<Function>),*
        }
        impl BuiltinFunctionTypes {
            pub fn new() -> Self {
                Self {
                    $($Name : Rc::new(Function{
                        args: vec![$($Arg),*],
                        return_type: $ReturnType,
                        arg_names: vec![],
                    })),*
                }
            }

            pub fn ty(&self, function: &BuiltinFunction) -> Rc<Function> {
                match function {
                    $(BuiltinFunction::$Name $(($Pattern))? => self.$Name.clone()),*
                }
            }
        }
    };
}

declare_builtin_function_types!(
    GetWindowScaleFactor: () -> Type::UnitProduct(vec![(Unit::Phx, 1), (Unit::Px, -1)]),
    GetWindowDefaultFontSize: () -> Type::LogicalLength,
    AnimationTick: () -> Type::Duration,
    Debug: (Type::String) -> Type::Void,
    Mod: (Type::Int32, Type::Int32) -> Type::Int32,
    Round: (Type::Float32) -> Type::Int32,
    Ceil: (Type::Float32) -> Type::Int32,
    Floor: (Type::Float32) -> Type::Int32,
    Sqrt: (Type::Float32) -> Type::Float32,
    Abs: (Type::Float32) -> Type::Float32,
    Cos: (Type::Angle) -> Type::Float32,
    Sin: (Type::Angle) -> Type::Float32,
    Tan: (Type::Angle) -> Type::Float32,
    ACos: (Type::Float32) -> Type::Angle,
    ASin: (Type::Float32) -> Type::Angle,
    ATan: (Type::Float32) -> Type::Angle,
    ATan2: (Type::Float32, Type::Float32) -> Type::Angle,
    Log: (Type::Float32, Type::Float32) -> Type::Float32,
    Ln: (Type::Float32) -> Type::Float32,
    Pow: (Type::Float32, Type::Float32) -> Type::Float32,
    Exp: (Type::Float32) -> Type::Float32,
    ToFixed: (Type::Float32, Type::Int32) -> Type::String,
    ToPrecision: (Type::Float32, Type::Int32) -> Type::String,
    SetFocusItem: (Type::ElementReference) -> Type::Void,
    ClearFocusItem: (Type::ElementReference) -> Type::Void,
    ShowPopupWindow: (Type::ElementReference) -> Type::Void,
    ClosePopupWindow: (Type::ElementReference) -> Type::Void,
    ShowPopupMenu: (Type::ElementReference, Type::Model, typeregister::logical_point_type()) -> Type::Void,
    SetSelectionOffsets: (Type::ElementReference, Type::Int32, Type::Int32) -> Type::Void,
    ItemFontMetrics: (Type::ElementReference) -> typeregister::font_metrics_type(),
    StringToFloat: (Type::String) -> Type::Float32,
    StringIsFloat: (Type::String) -> Type::Bool,
    StringIsEmpty: (Type::String) -> Type::Bool,
    StringCharacterCount: (Type::String) -> Type::Int32,
    StringToLowercase: (Type::String) -> Type::String,
    StringToUppercase: (Type::String) -> Type::String,
    ImplicitLayoutInfo(..): (Type::ElementReference) -> Type::Struct(typeregister::layout_info_type()),
    ColorRgbaStruct: (Type::Color) -> Type::Struct(Rc::new(Struct {
        fields: IntoIterator::into_iter([
            (SmolStr::new_static("red"), Type::Int32),
            (SmolStr::new_static("green"), Type::Int32),
            (SmolStr::new_static("blue"), Type::Int32),
            (SmolStr::new_static("alpha"), Type::Int32),
        ])
        .collect(),
        name: Some("Color".into()),
        node: None,
        rust_attributes: None,
    })),
    ColorHsvaStruct: (Type::Color) -> Type::Struct(Rc::new(Struct {
        fields: IntoIterator::into_iter([
            (SmolStr::new_static("hue"), Type::Float32),
            (SmolStr::new_static("saturation"), Type::Float32),
            (SmolStr::new_static("value"), Type::Float32),
            (SmolStr::new_static("alpha"), Type::Float32),
        ])
        .collect(),
        name: Some("Color".into()),
        node: None,
        rust_attributes: None,
    })),
    ColorBrighter: (Type::Brush, Type::Float32) -> Type::Brush,
    ColorDarker: (Type::Brush, Type::Float32) -> Type::Brush,
    ColorTransparentize: (Type::Brush, Type::Float32) -> Type::Brush,
    ColorWithAlpha: (Type::Brush, Type::Float32) -> Type::Brush,
    ColorMix: (Type::Color, Type::Color, Type::Float32) -> Type::Color,
    ImageSize: (Type::Image) -> Type::Struct(Rc::new(Struct {
        fields: IntoIterator::into_iter([
            (SmolStr::new_static("width"), Type::Int32),
            (SmolStr::new_static("height"), Type::Int32),
        ])
        .collect(),
        name: Some("Size".into()),
        node: None,
        rust_attributes: None,
    })),
    ArrayLength: (Type::Model) -> Type::Int32,
    Rgb: (Type::Int32, Type::Int32, Type::Int32, Type::Float32) -> Type::Color,
    Hsv: (Type::Float32, Type::Float32, Type::Float32, Type::Float32) -> Type::Color,
    ColorScheme: () -> Type::Enumeration(
        typeregister::BUILTIN.with(|e| e.enums.ColorScheme.clone()),
    ),
    SupportsNativeMenuBar: () -> Type::Bool,
    // entries, sub-menu, activate. But the types here are not accurate.
    SetupNativeMenuBar: (Type::Model, typeregister::noarg_callback_type(), typeregister::noarg_callback_type()) -> Type::Void,
    MonthDayCount: (Type::Int32, Type::Int32) -> Type::Int32,
    MonthOffset: (Type::Int32, Type::Int32) -> Type::Int32,
    FormatDate: (Type::String, Type::Int32, Type::Int32, Type::Int32) -> Type::String,
    TextInputFocused: () -> Type::Bool,
    DateNow: () -> Type::Array(Rc::new(Type::Int32)),
    ValidDate: (Type::String, Type::String) -> Type::Bool,
    ParseDate: (Type::String, Type::String) -> Type::Array(Rc::new(Type::Int32)),
    SetTextInputFocused: (Type::Bool) -> Type::Void,
    ItemAbsolutePosition: (Type::ElementReference) -> typeregister::logical_point_type(),
    RegisterCustomFontByPath: (Type::String) -> Type::Void,
    RegisterCustomFontByMemory: (Type::Int32) -> Type::Void,
    RegisterBitmapFont: (Type::Int32) -> Type::Void,
    // original, context, domain, args
    Translate: (Type::String, Type::String, Type::String, Type::Array(Type::String.into())) -> Type::String,
    Use24HourFormat: () -> Type::Bool,
    UpdateTimers: () -> Type::Void,
    DetectOperatingSystem: () -> Type::Enumeration(
        typeregister::BUILTIN.with(|e| e.enums.OperatingSystemType.clone()),
    ),
    StartTimer: (Type::ElementReference) -> Type::Void,
    StopTimer: (Type::ElementReference) -> Type::Void,
    RestartTimer: (Type::ElementReference) -> Type::Void,
);

impl BuiltinFunction {
    pub fn ty(&self) -> Rc<Function> {
        thread_local! {
            static TYPES: BuiltinFunctionTypes = BuiltinFunctionTypes::new();
        }
        TYPES.with(|types| types.ty(self))
    }

    /// It is const if the return value only depends on its argument and has no side effect
    fn is_const(&self) -> bool {
        match self {
            BuiltinFunction::GetWindowScaleFactor => false,
            BuiltinFunction::GetWindowDefaultFontSize => false,
            BuiltinFunction::AnimationTick => false,
            BuiltinFunction::ColorScheme => false,
            BuiltinFunction::SupportsNativeMenuBar => false,
            BuiltinFunction::SetupNativeMenuBar => false,
            BuiltinFunction::MonthDayCount => false,
            BuiltinFunction::MonthOffset => false,
            BuiltinFunction::FormatDate => false,
            BuiltinFunction::DateNow => false,
            BuiltinFunction::ValidDate => false,
            BuiltinFunction::ParseDate => false,
            // Even if it is not pure, we optimize it away anyway
            BuiltinFunction::Debug => true,
            BuiltinFunction::Mod
            | BuiltinFunction::Round
            | BuiltinFunction::Ceil
            | BuiltinFunction::Floor
            | BuiltinFunction::Abs
            | BuiltinFunction::Sqrt
            | BuiltinFunction::Cos
            | BuiltinFunction::Sin
            | BuiltinFunction::Tan
            | BuiltinFunction::ACos
            | BuiltinFunction::ASin
            | BuiltinFunction::Log
            | BuiltinFunction::Ln
            | BuiltinFunction::Pow
            | BuiltinFunction::Exp
            | BuiltinFunction::ATan
            | BuiltinFunction::ATan2
            | BuiltinFunction::ToFixed
            | BuiltinFunction::ToPrecision => true,
            BuiltinFunction::SetFocusItem | BuiltinFunction::ClearFocusItem => false,
            BuiltinFunction::ShowPopupWindow
            | BuiltinFunction::ClosePopupWindow
            | BuiltinFunction::ShowPopupMenu => false,
            BuiltinFunction::SetSelectionOffsets => false,
            BuiltinFunction::ItemFontMetrics => false, // depends also on Window's font properties
            BuiltinFunction::StringToFloat
            | BuiltinFunction::StringIsFloat
            | BuiltinFunction::StringIsEmpty
            | BuiltinFunction::StringCharacterCount
            | BuiltinFunction::StringToLowercase
            | BuiltinFunction::StringToUppercase => true,
            BuiltinFunction::ColorRgbaStruct
            | BuiltinFunction::ColorHsvaStruct
            | BuiltinFunction::ColorBrighter
            | BuiltinFunction::ColorDarker
            | BuiltinFunction::ColorTransparentize
            | BuiltinFunction::ColorMix
            | BuiltinFunction::ColorWithAlpha => true,
            // ImageSize is pure, except when loading images via the network. Then the initial size will be 0/0 and
            // we need to make sure that calls to this function stay within a binding, so that the property
            // notification when updating kicks in. Only SlintPad (wasm-interpreter) loads images via the network,
            // which is when this code is targeting wasm.
            #[cfg(not(target_arch = "wasm32"))]
            BuiltinFunction::ImageSize => true,
            #[cfg(target_arch = "wasm32")]
            BuiltinFunction::ImageSize => false,
            BuiltinFunction::ArrayLength => true,
            BuiltinFunction::Rgb => true,
            BuiltinFunction::Hsv => true,
            BuiltinFunction::SetTextInputFocused => false,
            BuiltinFunction::TextInputFocused => false,
            BuiltinFunction::ImplicitLayoutInfo(_) => false,
            BuiltinFunction::ItemAbsolutePosition => true,
            BuiltinFunction::RegisterCustomFontByPath
            | BuiltinFunction::RegisterCustomFontByMemory
            | BuiltinFunction::RegisterBitmapFont => false,
            BuiltinFunction::Translate => false,
            BuiltinFunction::Use24HourFormat => false,
            BuiltinFunction::UpdateTimers => false,
            BuiltinFunction::DetectOperatingSystem => true,
            BuiltinFunction::StartTimer => false,
            BuiltinFunction::StopTimer => false,
            BuiltinFunction::RestartTimer => false,
        }
    }

    // It is pure if it has no side effect
    pub fn is_pure(&self) -> bool {
        match self {
            BuiltinFunction::GetWindowScaleFactor => true,
            BuiltinFunction::GetWindowDefaultFontSize => true,
            BuiltinFunction::AnimationTick => true,
            BuiltinFunction::ColorScheme => true,
            BuiltinFunction::SupportsNativeMenuBar => true,
            BuiltinFunction::SetupNativeMenuBar => false,
            BuiltinFunction::MonthDayCount => true,
            BuiltinFunction::MonthOffset => true,
            BuiltinFunction::FormatDate => true,
            BuiltinFunction::DateNow => true,
            BuiltinFunction::ValidDate => true,
            BuiltinFunction::ParseDate => true,
            // Even if it has technically side effect, we still consider it as pure for our purpose
            BuiltinFunction::Debug => true,
            BuiltinFunction::Mod
            | BuiltinFunction::Round
            | BuiltinFunction::Ceil
            | BuiltinFunction::Floor
            | BuiltinFunction::Abs
            | BuiltinFunction::Sqrt
            | BuiltinFunction::Cos
            | BuiltinFunction::Sin
            | BuiltinFunction::Tan
            | BuiltinFunction::ACos
            | BuiltinFunction::ASin
            | BuiltinFunction::Log
            | BuiltinFunction::Ln
            | BuiltinFunction::Pow
            | BuiltinFunction::Exp
            | BuiltinFunction::ATan
            | BuiltinFunction::ATan2
            | BuiltinFunction::ToFixed
            | BuiltinFunction::ToPrecision => true,
            BuiltinFunction::SetFocusItem | BuiltinFunction::ClearFocusItem => false,
            BuiltinFunction::ShowPopupWindow
            | BuiltinFunction::ClosePopupWindow
            | BuiltinFunction::ShowPopupMenu => false,
            BuiltinFunction::SetSelectionOffsets => false,
            BuiltinFunction::ItemFontMetrics => true,
            BuiltinFunction::StringToFloat
            | BuiltinFunction::StringIsFloat
            | BuiltinFunction::StringIsEmpty
            | BuiltinFunction::StringCharacterCount
            | BuiltinFunction::StringToLowercase
            | BuiltinFunction::StringToUppercase => true,
            BuiltinFunction::ColorRgbaStruct
            | BuiltinFunction::ColorHsvaStruct
            | BuiltinFunction::ColorBrighter
            | BuiltinFunction::ColorDarker
            | BuiltinFunction::ColorTransparentize
            | BuiltinFunction::ColorMix
            | BuiltinFunction::ColorWithAlpha => true,
            BuiltinFunction::ImageSize => true,
            BuiltinFunction::ArrayLength => true,
            BuiltinFunction::Rgb => true,
            BuiltinFunction::Hsv => true,
            BuiltinFunction::ImplicitLayoutInfo(_) => true,
            BuiltinFunction::ItemAbsolutePosition => true,
            BuiltinFunction::SetTextInputFocused => false,
            BuiltinFunction::TextInputFocused => true,
            BuiltinFunction::RegisterCustomFontByPath
            | BuiltinFunction::RegisterCustomFontByMemory
            | BuiltinFunction::RegisterBitmapFont => false,
            BuiltinFunction::Translate => true,
            BuiltinFunction::Use24HourFormat => true,
            BuiltinFunction::UpdateTimers => false,
            BuiltinFunction::DetectOperatingSystem => true,
            BuiltinFunction::StartTimer => false,
            BuiltinFunction::StopTimer => false,
            BuiltinFunction::RestartTimer => false,
        }
    }
}

/// The base of a Expression::FunctionCall
#[derive(Debug, Clone)]
pub enum Callable {
    Callback(NamedReference),
    Function(NamedReference),
    Builtin(BuiltinFunction),
}
impl Callable {
    pub fn ty(&self) -> Type {
        match self {
            Callable::Callback(nr) => nr.ty(),
            Callable::Function(nr) => nr.ty(),
            Callable::Builtin(b) => Type::Function(b.ty()),
        }
    }
}
impl From<BuiltinFunction> for Callable {
    fn from(function: BuiltinFunction) -> Self {
        Self::Builtin(function)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum OperatorClass {
    ComparisonOp,
    LogicalOp,
    ArithmeticOp,
}

/// the class of for this (binary) operation
pub fn operator_class(op: char) -> OperatorClass {
    match op {
        '=' | '!' | '<' | '>' | '≤' | '≥' => OperatorClass::ComparisonOp,
        '&' | '|' => OperatorClass::LogicalOp,
        '+' | '-' | '/' | '*' => OperatorClass::ArithmeticOp,
        _ => panic!("Invalid operator {op:?}"),
    }
}

macro_rules! declare_units {
    ($( $(#[$m:meta])* $ident:ident = $string:literal -> $ty:ident $(* $factor:expr)? ,)*) => {
        /// The units that can be used after numbers in the language
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, strum::EnumIter)]
        pub enum Unit {
            $($(#[$m])* $ident,)*
        }

        impl std::fmt::Display for Unit {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                match self {
                    $(Self::$ident => write!(f, $string), )*
                }
            }
        }

        impl std::str::FromStr for Unit {
            type Err = ();
            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($string => Ok(Self::$ident), )*
                    _ => Err(())
                }
            }
        }

        impl Unit {
            pub fn ty(self) -> Type {
                match self {
                    $(Self::$ident => Type::$ty, )*
                }
            }

            pub fn normalize(self, x: f64) -> f64 {
                match self {
                    $(Self::$ident => x $(* $factor as f64)?, )*
                }
            }

        }
    };
}

declare_units! {
    /// No unit was given
    None = "" -> Float32,
    /// Percent value
    Percent = "%" -> Percent,

    // Lengths or Coord

    /// Physical pixels
    Phx = "phx" -> PhysicalLength,
    /// Logical pixels
    Px = "px" -> LogicalLength,
    /// Centimeters
    Cm = "cm" -> LogicalLength * 37.8,
    /// Millimeters
    Mm = "mm" -> LogicalLength * 3.78,
    /// inches
    In = "in" -> LogicalLength * 96,
    /// Points
    Pt = "pt" -> LogicalLength * 96./72.,
    /// Logical pixels multiplied with the window's default-font-size
    Rem = "rem" -> Rem,

    // durations

    /// Seconds
    S = "s" -> Duration * 1000,
    /// Milliseconds
    Ms = "ms" -> Duration,

    // angles

    /// Degree
    Deg = "deg" -> Angle,
    /// Gradians
    Grad = "grad" -> Angle * 360./180.,
    /// Turns
    Turn = "turn" -> Angle * 360.,
    /// Radians
    Rad = "rad" -> Angle * 360./std::f32::consts::TAU,
}

impl Default for Unit {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MinMaxOp {
    Min,
    Max,
}

/// The Expression is hold by properties, so it should not hold any strong references to node from the object_tree
#[derive(Debug, Clone, Default)]
pub enum Expression {
    /// Something went wrong (and an error will be reported)
    #[default]
    Invalid,
    /// We haven't done the lookup yet
    Uncompiled(SyntaxNode),

    /// A string literal. The .0 is the content of the string, without the quotes
    StringLiteral(SmolStr),
    /// Number
    NumberLiteral(f64, Unit),
    /// Bool
    BoolLiteral(bool),

    /// Reference to the property
    PropertyReference(NamedReference),

    /// A reference to a specific element. This isn't possible to create in .slint syntax itself, but intermediate passes may generate this
    /// type of expression.
    ElementReference(Weak<RefCell<Element>>),

    /// Reference to the index variable of a repeater
    ///
    /// Example: `idx`  in `for xxx[idx] in ...`.   The element is the reference to the
    /// element that is repeated
    RepeaterIndexReference {
        element: Weak<RefCell<Element>>,
    },

    /// Reference to the model variable of a repeater
    ///
    /// Example: `xxx`  in `for xxx[idx] in ...`.   The element is the reference to the
    /// element that is repeated
    RepeaterModelReference {
        element: Weak<RefCell<Element>>,
    },

    /// Reference the parameter at the given index of the current function.
    FunctionParameterReference {
        index: usize,
        ty: Type,
    },

    /// Should be directly within a CodeBlock expression, and store the value of the expression in a local variable
    StoreLocalVariable {
        name: SmolStr,
        value: Box<Expression>,
    },

    /// a reference to the local variable with the given name. The type system should ensure that a variable has been stored
    /// with this name and this type before in one of the statement of an enclosing codeblock
    ReadLocalVariable {
        name: SmolStr,
        ty: Type,
    },

    /// Access to a field of the given name within a struct.
    StructFieldAccess {
        /// This expression should have [`Type::Struct`] type
        base: Box<Expression>,
        name: SmolStr,
    },

    /// Access to a index within an array.
    ArrayIndex {
        /// This expression should have [`Type::Array`] type
        array: Box<Expression>,
        index: Box<Expression>,
    },

    /// Cast an expression to the given type
    Cast {
        from: Box<Expression>,
        to: Type,
    },

    /// a code block with different expression
    CodeBlock(Vec<Expression>),

    /// A function call
    FunctionCall {
        function: Callable,
        arguments: Vec<Expression>,
        source_location: Option<SourceLocation>,
    },

    /// A SelfAssignment or an Assignment.  When op is '=' this is a simple assignment.
    SelfAssignment {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
        /// '+', '-', '/', '*', or '='
        op: char,
        node: Option<NodeOrToken>,
    },

    BinaryExpression {
        lhs: Box<Expression>,
        rhs: Box<Expression>,
        /// '+', '-', '/', '*', '=', '!', '<', '>', '≤', '≥', '&', '|'
        op: char,
    },

    UnaryOp {
        sub: Box<Expression>,
        /// '+', '-', '!'
        op: char,
    },

    ImageReference {
        resource_ref: ImageReference,
        source_location: Option<SourceLocation>,
        nine_slice: Option<[u16; 4]>,
    },

    Condition {
        condition: Box<Expression>,
        true_expr: Box<Expression>,
        false_expr: Box<Expression>,
    },

    Array {
        element_ty: Type,
        values: Vec<Expression>,
    },
    Struct {
        ty: Rc<Struct>,
        values: HashMap<SmolStr, Expression>,
    },

    PathData(Path),

    EasingCurve(EasingCurve),

    LinearGradient {
        angle: Box<Expression>,
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    RadialGradient {
        /// First expression in the tuple is a color, second expression is the stop position
        stops: Vec<(Expression, Expression)>,
    },

    EnumerationValue(EnumerationValue),

    ReturnStatement(Option<Box<Expression>>),

    LayoutCacheAccess {
        layout_cache_prop: NamedReference,
        index: usize,
        /// When set, this is the index within a repeater, and the index is then the location of another offset.
        /// So this looks like `layout_cache_prop[layout_cache_prop[index] + repeater_index]`
        repeater_index: Option<Box<Expression>>,
    },
    /// Compute the LayoutInfo for the given layout.
    /// The orientation is the orientation of the cache, not the orientation of the layout
    ComputeLayoutInfo(crate::layout::Layout, crate::layout::Orientation),
    SolveLayout(crate::layout::Layout, crate::layout::Orientation),

    MinMax {
        ty: Type,
        op: MinMaxOp,
        lhs: Box<Expression>,
        rhs: Box<Expression>,
    },

    DebugHook {
        expression: Box<Expression>,
        id: SmolStr,
    },

    EmptyComponentFactory,
}

impl Expression {
    /// Return the type of this property
    pub fn ty(&self) -> Type {
        match self {
            Expression::Invalid => Type::Invalid,
            Expression::Uncompiled(_) => Type::Invalid,
            Expression::StringLiteral(_) => Type::String,
            Expression::NumberLiteral(_, unit) => unit.ty(),
            Expression::BoolLiteral(_) => Type::Bool,
            Expression::PropertyReference(nr) => nr.ty(),
            Expression::ElementReference(_) => Type::ElementReference,
            Expression::RepeaterIndexReference { .. } => Type::Int32,
            Expression::RepeaterModelReference { element } => element
                .upgrade()
                .unwrap()
                .borrow()
                .repeated
                .as_ref()
                .map_or(Type::Invalid, |e| model_inner_type(&e.model)),
            Expression::FunctionParameterReference { ty, .. } => ty.clone(),
            Expression::StructFieldAccess { base, name } => match base.ty() {
                Type::Struct(s) => s.fields.get(name.as_str()).unwrap_or(&Type::Invalid).clone(),
                _ => Type::Invalid,
            },
            Expression::ArrayIndex { array, .. } => match array.ty() {
                Type::Array(ty) => (*ty).clone(),
                _ => Type::Invalid,
            },
            Expression::Cast { to, .. } => to.clone(),
            Expression::CodeBlock(sub) => sub.last().map_or(Type::Void, |e| e.ty()),
            Expression::FunctionCall { function, .. } => match function.ty() {
                Type::Function(f) | Type::Callback(f) => f.return_type.clone(),
                _ => Type::Invalid,
            },
            Expression::SelfAssignment { .. } => Type::Void,
            Expression::ImageReference { .. } => Type::Image,
            Expression::Condition { condition: _, true_expr, false_expr } => {
                let true_type = true_expr.ty();
                let false_type = false_expr.ty();
                if true_type == false_type {
                    true_type
                } else if true_type == Type::Invalid {
                    false_type
                } else if false_type == Type::Invalid {
                    true_type
                } else {
                    Type::Void
                }
            }
            Expression::BinaryExpression { op, lhs, rhs } => {
                if operator_class(*op) != OperatorClass::ArithmeticOp {
                    Type::Bool
                } else if *op == '+' || *op == '-' {
                    let (rhs_ty, lhs_ty) = (rhs.ty(), lhs.ty());
                    if rhs_ty == lhs_ty {
                        rhs_ty
                    } else {
                        Type::Invalid
                    }
                } else {
                    debug_assert!(*op == '*' || *op == '/');
                    let unit_vec = |ty| {
                        if let Type::UnitProduct(v) = ty {
                            v
                        } else if let Some(u) = ty.default_unit() {
                            vec![(u, 1)]
                        } else {
                            vec![]
                        }
                    };
                    let mut l_units = unit_vec(lhs.ty());
                    let mut r_units = unit_vec(rhs.ty());
                    if *op == '/' {
                        for (_, power) in &mut r_units {
                            *power = -*power;
                        }
                    }
                    for (unit, power) in r_units {
                        if let Some((_, p)) = l_units.iter_mut().find(|(u, _)| *u == unit) {
                            *p += power;
                        } else {
                            l_units.push((unit, power));
                        }
                    }

                    // normalize the vector by removing empty and sorting
                    l_units.retain(|(_, p)| *p != 0);
                    l_units.sort_unstable_by(|(u1, p1), (u2, p2)| match p2.cmp(p1) {
                        std::cmp::Ordering::Equal => u1.cmp(u2),
                        x => x,
                    });

                    if l_units.is_empty() {
                        Type::Float32
                    } else if l_units.len() == 1 && l_units[0].1 == 1 {
                        l_units[0].0.ty()
                    } else {
                        Type::UnitProduct(l_units)
                    }
                }
            }
            Expression::UnaryOp { sub, .. } => sub.ty(),
            Expression::Array { element_ty, .. } => Type::Array(Rc::new(element_ty.clone())),
            Expression::Struct { ty, .. } => ty.clone().into(),
            Expression::PathData { .. } => Type::PathData,
            Expression::StoreLocalVariable { .. } => Type::Void,
            Expression::ReadLocalVariable { ty, .. } => ty.clone(),
            Expression::EasingCurve(_) => Type::Easing,
            Expression::LinearGradient { .. } => Type::Brush,
            Expression::RadialGradient { .. } => Type::Brush,
            Expression::EnumerationValue(value) => Type::Enumeration(value.enumeration.clone()),
            // invalid because the expression is unreachable
            Expression::ReturnStatement(_) => Type::Invalid,
            Expression::LayoutCacheAccess { .. } => Type::LogicalLength,
            Expression::ComputeLayoutInfo(..) => typeregister::layout_info_type().into(),
            Expression::SolveLayout(..) => Type::LayoutCache,
            Expression::MinMax { ty, .. } => ty.clone(),
            Expression::EmptyComponentFactory => Type::ComponentFactory,
            Expression::DebugHook { expression, .. } => expression.ty(),
        }
    }

    /// Call the visitor for each sub-expression.  (note: this function does not recurse)
    pub fn visit(&self, mut visitor: impl FnMut(&Self)) {
        match self {
            Expression::Invalid => {}
            Expression::Uncompiled(_) => {}
            Expression::StringLiteral(_) => {}
            Expression::NumberLiteral(_, _) => {}
            Expression::BoolLiteral(_) => {}
            Expression::PropertyReference { .. } => {}
            Expression::FunctionParameterReference { .. } => {}
            Expression::ElementReference(_) => {}
            Expression::StructFieldAccess { base, .. } => visitor(base),
            Expression::ArrayIndex { array, index } => {
                visitor(array);
                visitor(index);
            }
            Expression::RepeaterIndexReference { .. } => {}
            Expression::RepeaterModelReference { .. } => {}
            Expression::Cast { from, .. } => visitor(from),
            Expression::CodeBlock(sub) => {
                sub.iter().for_each(visitor);
            }
            Expression::FunctionCall { function: _, arguments, source_location: _ } => {
                arguments.iter().for_each(visitor);
            }
            Expression::SelfAssignment { lhs, rhs, .. } => {
                visitor(lhs);
                visitor(rhs);
            }
            Expression::ImageReference { .. } => {}
            Expression::Condition { condition, true_expr, false_expr } => {
                visitor(condition);
                visitor(true_expr);
                visitor(false_expr);
            }
            Expression::BinaryExpression { lhs, rhs, .. } => {
                visitor(lhs);
                visitor(rhs);
            }
            Expression::UnaryOp { sub, .. } => visitor(sub),
            Expression::Array { values, .. } => {
                for x in values {
                    visitor(x);
                }
            }
            Expression::Struct { values, .. } => {
                for x in values.values() {
                    visitor(x);
                }
            }
            Expression::PathData(data) => match data {
                Path::Elements(elements) => {
                    for element in elements {
                        element.bindings.values().for_each(|binding| visitor(&binding.borrow()))
                    }
                }
                Path::Events(events, coordinates) => {
                    events.iter().chain(coordinates.iter()).for_each(visitor);
                }
                Path::Commands(commands) => visitor(commands),
            },
            Expression::StoreLocalVariable { value, .. } => visitor(value),
            Expression::ReadLocalVariable { .. } => {}
            Expression::EasingCurve(_) => {}
            Expression::LinearGradient { angle, stops } => {
                visitor(angle);
                for (c, s) in stops {
                    visitor(c);
                    visitor(s);
                }
            }
            Expression::RadialGradient { stops } => {
                for (c, s) in stops {
                    visitor(c);
                    visitor(s);
                }
            }
            Expression::EnumerationValue(_) => {}
            Expression::ReturnStatement(expr) => {
                expr.as_deref().map(visitor);
            }
            Expression::LayoutCacheAccess { repeater_index, .. } => {
                repeater_index.as_deref().map(visitor);
            }
            Expression::ComputeLayoutInfo(..) => {}
            Expression::SolveLayout(..) => {}
            Expression::MinMax { lhs, rhs, .. } => {
                visitor(lhs);
                visitor(rhs);
            }
            Expression::EmptyComponentFactory => {}
            Expression::DebugHook { expression, .. } => visitor(expression),
        }
    }

    pub fn visit_mut(&mut self, mut visitor: impl FnMut(&mut Self)) {
        match self {
            Expression::Invalid => {}
            Expression::Uncompiled(_) => {}
            Expression::StringLiteral(_) => {}
            Expression::NumberLiteral(_, _) => {}
            Expression::BoolLiteral(_) => {}
            Expression::PropertyReference { .. } => {}
            Expression::FunctionParameterReference { .. } => {}
            Expression::ElementReference(_) => {}
            Expression::StructFieldAccess { base, .. } => visitor(base),
            Expression::ArrayIndex { array, index } => {
                visitor(array);
                visitor(index);
            }
            Expression::RepeaterIndexReference { .. } => {}
            Expression::RepeaterModelReference { .. } => {}
            Expression::Cast { from, .. } => visitor(from),
            Expression::CodeBlock(sub) => {
                sub.iter_mut().for_each(visitor);
            }
            Expression::FunctionCall { function: _, arguments, source_location: _ } => {
                arguments.iter_mut().for_each(visitor);
            }
            Expression::SelfAssignment { lhs, rhs, .. } => {
                visitor(lhs);
                visitor(rhs);
            }
            Expression::ImageReference { .. } => {}
            Expression::Condition { condition, true_expr, false_expr } => {
                visitor(condition);
                visitor(true_expr);
                visitor(false_expr);
            }
            Expression::BinaryExpression { lhs, rhs, .. } => {
                visitor(lhs);
                visitor(rhs);
            }
            Expression::UnaryOp { sub, .. } => visitor(sub),
            Expression::Array { values, .. } => {
                for x in values {
                    visitor(x);
                }
            }
            Expression::Struct { values, .. } => {
                for x in values.values_mut() {
                    visitor(x);
                }
            }
            Expression::PathData(data) => match data {
                Path::Elements(elements) => {
                    for element in elements {
                        element
                            .bindings
                            .values_mut()
                            .for_each(|binding| visitor(&mut binding.borrow_mut()))
                    }
                }
                Path::Events(events, coordinates) => {
                    events.iter_mut().chain(coordinates.iter_mut()).for_each(visitor);
                }
                Path::Commands(commands) => visitor(commands),
            },
            Expression::StoreLocalVariable { value, .. } => visitor(value),
            Expression::ReadLocalVariable { .. } => {}
            Expression::EasingCurve(_) => {}
            Expression::LinearGradient { angle, stops } => {
                visitor(angle);
                for (c, s) in stops {
                    visitor(c);
                    visitor(s);
                }
            }
            Expression::RadialGradient { stops } => {
                for (c, s) in stops {
                    visitor(c);
                    visitor(s);
                }
            }
            Expression::EnumerationValue(_) => {}
            Expression::ReturnStatement(expr) => {
                expr.as_deref_mut().map(visitor);
            }
            Expression::LayoutCacheAccess { repeater_index, .. } => {
                repeater_index.as_deref_mut().map(visitor);
            }
            Expression::ComputeLayoutInfo(..) => {}
            Expression::SolveLayout(..) => {}
            Expression::MinMax { lhs, rhs, .. } => {
                visitor(lhs);
                visitor(rhs);
            }
            Expression::EmptyComponentFactory => {}
            Expression::DebugHook { expression, .. } => visitor(expression),
        }
    }

    /// Visit itself and each sub expression recursively
    pub fn visit_recursive(&self, visitor: &mut dyn FnMut(&Self)) {
        visitor(self);
        self.visit(|e| e.visit_recursive(visitor));
    }

    /// Visit itself and each sub expression recursively
    pub fn visit_recursive_mut(&mut self, visitor: &mut dyn FnMut(&mut Self)) {
        visitor(self);
        self.visit_mut(|e| e.visit_recursive_mut(visitor));
    }

    pub fn is_constant(&self) -> bool {
        match self {
            Expression::Invalid => true,
            Expression::Uncompiled(_) => false,
            Expression::StringLiteral(_) => true,
            Expression::NumberLiteral(_, _) => true,
            Expression::BoolLiteral(_) => true,
            Expression::PropertyReference(nr) => nr.is_constant(),
            Expression::ElementReference(_) => false,
            Expression::RepeaterIndexReference { .. } => false,
            Expression::RepeaterModelReference { .. } => false,
            // Allow functions to be marked as const
            Expression::FunctionParameterReference { .. } => true,
            Expression::StructFieldAccess { base, .. } => base.is_constant(),
            Expression::ArrayIndex { array, index } => array.is_constant() && index.is_constant(),
            Expression::Cast { from, .. } => from.is_constant(),
            // This is conservative: the return value is the last expression in the block, but
            // we kind of mean "pure" here too, so ensure the whole body is OK.
            Expression::CodeBlock(sub) => sub.iter().all(|s| s.is_constant()),
            Expression::FunctionCall { function, arguments, .. } => {
                let is_const = match function {
                    Callable::Builtin(b) => b.is_const(),
                    Callable::Function(nr) => nr.is_constant(),
                    Callable::Callback(..) => false,
                };
                is_const && arguments.iter().all(|a| a.is_constant())
            }
            Expression::SelfAssignment { .. } => false,
            Expression::ImageReference { .. } => true,
            Expression::Condition { condition, false_expr, true_expr } => {
                condition.is_constant() && false_expr.is_constant() && true_expr.is_constant()
            }
            Expression::BinaryExpression { lhs, rhs, .. } => lhs.is_constant() && rhs.is_constant(),
            Expression::UnaryOp { sub, .. } => sub.is_constant(),
            // Array will turn into model, and they can't be considered as constant if the model
            // is used and the model is changed. CF issue #5249
            //Expression::Array { values, .. } => values.iter().all(Expression::is_constant),
            Expression::Array { .. } => false,
            Expression::Struct { values, .. } => values.iter().all(|(_, v)| v.is_constant()),
            Expression::PathData(data) => match data {
                Path::Elements(elements) => elements
                    .iter()
                    .all(|element| element.bindings.values().all(|v| v.borrow().is_constant())),
                Path::Events(_, _) => true,
                Path::Commands(_) => false,
            },
            Expression::StoreLocalVariable { value, .. } => value.is_constant(),
            // We only load what we store, and stores are alredy checked
            Expression::ReadLocalVariable { .. } => true,
            Expression::EasingCurve(_) => true,
            Expression::LinearGradient { angle, stops } => {
                angle.is_constant() && stops.iter().all(|(c, s)| c.is_constant() && s.is_constant())
            }
            Expression::RadialGradient { stops } => {
                stops.iter().all(|(c, s)| c.is_constant() && s.is_constant())
            }
            Expression::EnumerationValue(_) => true,
            Expression::ReturnStatement(expr) => {
                expr.as_ref().map_or(true, |expr| expr.is_constant())
            }
            // TODO:  detect constant property within layouts
            Expression::LayoutCacheAccess { .. } => false,
            Expression::ComputeLayoutInfo(..) => false,
            Expression::SolveLayout(..) => false,
            Expression::MinMax { lhs, rhs, .. } => lhs.is_constant() && rhs.is_constant(),
            Expression::EmptyComponentFactory => true,
            Expression::DebugHook { .. } => false,
        }
    }

    /// Create a conversion node if needed, or throw an error if the type is not matching
    #[must_use]
    pub fn maybe_convert_to(
        self,
        target_type: Type,
        node: &dyn Spanned,
        diag: &mut BuildDiagnostics,
    ) -> Expression {
        let ty = self.ty();
        if ty == target_type
            || target_type == Type::Void
            || target_type == Type::Invalid
            || ty == Type::Invalid
        {
            self
        } else if ty.can_convert(&target_type) {
            let from = match (ty, &target_type) {
                (Type::Brush, Type::Color) => match self {
                    Expression::LinearGradient { .. } | Expression::RadialGradient { .. } => {
                        let message = format!("Narrowing conversion from {0} to {1}. This can lead to unexpected behavior because the {0} is a gradient", Type::Brush, Type::Color);
                        diag.push_warning(message, node);
                        self
                    }
                    _ => self,
                },
                (Type::Percent, Type::Float32) => Expression::BinaryExpression {
                    lhs: Box::new(self),
                    rhs: Box::new(Expression::NumberLiteral(0.01, Unit::None)),
                    op: '*',
                },
                (ref from_ty @ Type::Struct(ref left), Type::Struct(right))
                    if left.fields != right.fields =>
                {
                    if let Expression::Struct { mut values, .. } = self {
                        let mut new_values = HashMap::new();
                        for (key, ty) in &right.fields {
                            let (key, expression) = values.remove_entry(key).map_or_else(
                                || (key.clone(), Expression::default_value_for_type(ty)),
                                |(k, e)| (k, e.maybe_convert_to(ty.clone(), node, diag)),
                            );
                            new_values.insert(key, expression);
                        }
                        return Expression::Struct { values: new_values, ty: right.clone() };
                    }
                    static COUNT: std::sync::atomic::AtomicUsize =
                        std::sync::atomic::AtomicUsize::new(0);
                    let var_name = format_smolstr!(
                        "tmpobj_conv_{}",
                        COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    );
                    let mut new_values = HashMap::new();
                    for (key, ty) in &right.fields {
                        let expression = if left.fields.contains_key(key) {
                            Expression::StructFieldAccess {
                                base: Box::new(Expression::ReadLocalVariable {
                                    name: var_name.clone(),
                                    ty: from_ty.clone(),
                                }),
                                name: key.clone(),
                            }
                            .maybe_convert_to(ty.clone(), node, diag)
                        } else {
                            Expression::default_value_for_type(ty)
                        };
                        new_values.insert(key.clone(), expression);
                    }
                    return Expression::CodeBlock(vec![
                        Expression::StoreLocalVariable { name: var_name, value: Box::new(self) },
                        Expression::Struct { values: new_values, ty: right.clone() },
                    ]);
                }
                (left, right) => match (left.as_unit_product(), right.as_unit_product()) {
                    (Some(left), Some(right)) => {
                        if let Some(conversion_powers) =
                            crate::langtype::unit_product_length_conversion(&left, &right)
                        {
                            let apply_power =
                                |mut result, power: i8, builtin_fn: BuiltinFunction| {
                                    let op = if power < 0 { '*' } else { '/' };
                                    for _ in 0..power.abs() {
                                        result = Expression::BinaryExpression {
                                            lhs: Box::new(result),
                                            rhs: Box::new(Expression::FunctionCall {
                                                function: Callable::Builtin(builtin_fn.clone()),
                                                arguments: vec![],
                                                source_location: Some(node.to_source_location()),
                                            }),
                                            op,
                                        }
                                    }
                                    result
                                };

                            let mut result = self;

                            if conversion_powers.rem_to_px_power != 0 {
                                result = apply_power(
                                    result,
                                    conversion_powers.rem_to_px_power,
                                    BuiltinFunction::GetWindowDefaultFontSize,
                                )
                            }
                            if conversion_powers.px_to_phx_power != 0 {
                                result = apply_power(
                                    result,
                                    conversion_powers.px_to_phx_power,
                                    BuiltinFunction::GetWindowScaleFactor,
                                )
                            }

                            result
                        } else {
                            self
                        }
                    }
                    _ => self,
                },
            };
            Expression::Cast { from: Box::new(from), to: target_type }
        } else if matches!(
            (&ty, &target_type, &self),
            (Type::Array(_), Type::Array(_), Expression::Array { .. })
        ) {
            // Special case for converting array literals
            match (self, target_type) {
                (Expression::Array { values, .. }, Type::Array(target_type)) => Expression::Array {
                    values: values
                        .into_iter()
                        .map(|e| e.maybe_convert_to((*target_type).clone(), node, diag))
                        .take_while(|e| !matches!(e, Expression::Invalid))
                        .collect(),
                    element_ty: (*target_type).clone(),
                },
                _ => unreachable!(),
            }
        } else if let (Type::Struct(struct_type), Expression::Struct { values, .. }) =
            (&target_type, &self)
        {
            // Also special case struct literal in case they contain array literal
            let mut fields = struct_type.fields.clone();
            let mut new_values = HashMap::new();
            for (f, v) in values {
                if let Some(t) = fields.remove(f) {
                    new_values.insert(f.clone(), v.clone().maybe_convert_to(t, node, diag));
                } else {
                    diag.push_error(format!("Cannot convert {ty} to {target_type}"), node);
                    return self;
                }
            }
            for (f, t) in fields {
                new_values.insert(f, Expression::default_value_for_type(&t));
            }
            Expression::Struct { ty: struct_type.clone(), values: new_values }
        } else {
            let mut message = format!("Cannot convert {ty} to {target_type}");
            // Explicit error message for unit conversion
            if let Some(from_unit) = ty.default_unit() {
                if matches!(&target_type, Type::Int32 | Type::Float32 | Type::String) {
                    message =
                        format!("{message}. Divide by 1{from_unit} to convert to a plain number");
                }
            } else if let Some(to_unit) = target_type.default_unit() {
                if matches!(ty, Type::Int32 | Type::Float32) {
                    if let Expression::NumberLiteral(value, Unit::None) = self {
                        if value == 0. {
                            // Allow conversion from literal 0 to any unit
                            return Expression::NumberLiteral(0., to_unit);
                        }
                    }
                    message = format!(
                        "{message}. Use an unit, or multiply by 1{to_unit} to convert explicitly"
                    );
                }
            }
            diag.push_error(message, node);
            self
        }
    }

    /// Return the default value for the given type
    pub fn default_value_for_type(ty: &Type) -> Expression {
        match ty {
            Type::Invalid
            | Type::Callback { .. }
            | Type::Function { .. }
            | Type::InferredProperty
            | Type::InferredCallback
            | Type::ElementReference
            | Type::LayoutCache => Expression::Invalid,
            Type::Void => Expression::CodeBlock(vec![]),
            Type::Float32 => Expression::NumberLiteral(0., Unit::None),
            Type::String => Expression::StringLiteral(SmolStr::default()),
            Type::Int32 | Type::Color | Type::UnitProduct(_) => Expression::Cast {
                from: Box::new(Expression::NumberLiteral(0., Unit::None)),
                to: ty.clone(),
            },
            Type::Duration => Expression::NumberLiteral(0., Unit::Ms),
            Type::Angle => Expression::NumberLiteral(0., Unit::Deg),
            Type::PhysicalLength => Expression::NumberLiteral(0., Unit::Phx),
            Type::LogicalLength => Expression::NumberLiteral(0., Unit::Px),
            Type::Rem => Expression::NumberLiteral(0., Unit::Rem),
            Type::Percent => Expression::NumberLiteral(100., Unit::Percent),
            Type::Image => Expression::ImageReference {
                resource_ref: ImageReference::None,
                source_location: None,
                nine_slice: None,
            },
            Type::Bool => Expression::BoolLiteral(false),
            Type::Model => Expression::Invalid,
            Type::PathData => Expression::PathData(Path::Elements(vec![])),
            Type::Array(element_ty) => {
                Expression::Array { element_ty: (**element_ty).clone(), values: vec![] }
            }
            Type::Struct(s) => Expression::Struct {
                ty: s.clone(),
                values: s
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), Expression::default_value_for_type(v)))
                    .collect(),
            },
            Type::Easing => Expression::EasingCurve(EasingCurve::default()),
            Type::Brush => Expression::Cast {
                from: Box::new(Expression::default_value_for_type(&Type::Color)),
                to: Type::Brush,
            },
            Type::Enumeration(enumeration) => {
                Expression::EnumerationValue(enumeration.clone().default_value())
            }
            Type::ComponentFactory => Expression::EmptyComponentFactory,
        }
    }

    /// Try to mark this expression to a lvalue that can be assigned to.
    ///
    /// Return true if the expression is a "lvalue" that can be used as the left hand side of a `=` or `+=` or similar
    pub fn try_set_rw(
        &mut self,
        ctx: &mut LookupCtx,
        what: &'static str,
        node: &dyn Spanned,
    ) -> bool {
        match self {
            Expression::PropertyReference(nr) => {
                nr.mark_as_set();
                let mut lookup = nr.element().borrow().lookup_property(nr.name());
                lookup.is_local_to_component &= ctx.is_local_element(&nr.element());
                if lookup.property_visibility == PropertyVisibility::Constexpr {
                    ctx.diag.push_error(
                        "The property must be known at compile time and cannot be changed at runtime"
                            .into(),
                        node,
                    );
                    false
                } else if lookup.is_valid_for_assignment() {
                    if !nr
                        .element()
                        .borrow()
                        .property_analysis
                        .borrow()
                        .get(nr.name())
                        .is_some_and(|d| d.is_linked_to_read_only)
                    {
                        true
                    } else if ctx.is_legacy_component() {
                        ctx.diag.push_warning("Modifying a property that is linked to a read-only property is deprecated".into(), node);
                        true
                    } else {
                        ctx.diag.push_error(
                            "Cannot modify a property that is linked to a read-only property"
                                .into(),
                            node,
                        );
                        false
                    }
                } else if ctx.is_legacy_component()
                    && lookup.property_visibility == PropertyVisibility::Output
                {
                    ctx.diag
                        .push_warning(format!("{what} on an output property is deprecated"), node);
                    true
                } else {
                    ctx.diag.push_error(
                        format!("{what} on a {} property", lookup.property_visibility),
                        node,
                    );
                    false
                }
            }
            Expression::StructFieldAccess { base, .. } => base.try_set_rw(ctx, what, node),
            Expression::RepeaterModelReference { .. } => true,
            Expression::ArrayIndex { array, .. } => array.try_set_rw(ctx, what, node),
            _ => {
                ctx.diag.push_error(format!("{what} needs to be done on a property"), node);
                false
            }
        }
    }

    /// Unwrap DebugHook expressions to their contained sub-expression
    pub fn ignore_debug_hooks(&self) -> &Expression {
        match self {
            Expression::DebugHook { expression, .. } => expression.as_ref(),
            _ => self,
        }
    }
}

fn model_inner_type(model: &Expression) -> Type {
    match model {
        Expression::Cast { from, to: Type::Model } => model_inner_type(from),
        Expression::CodeBlock(cb) => cb.last().map_or(Type::Invalid, model_inner_type),
        _ => match model.ty() {
            Type::Float32 | Type::Int32 => Type::Int32,
            Type::Array(elem) => (*elem).clone(),
            _ => Type::Invalid,
        },
    }
}

/// The expression in the Element::binding hash table
#[derive(Debug, Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct BindingExpression {
    #[deref]
    #[deref_mut]
    pub expression: Expression,
    /// The location of this expression in the source code
    pub span: Option<SourceLocation>,
    /// How deep is this binding declared in the hierarchy. When two binding are conflicting
    /// for the same priority (because of two way binding), the lower priority wins.
    /// The priority starts at 1, and each level of inlining adds one to the priority.
    /// 0 means the expression was added by some passes and it is not explicit in the source code
    pub priority: i32,

    pub animation: Option<PropertyAnimation>,

    /// The analysis information. None before it is computed
    pub analysis: Option<BindingAnalysis>,

    /// The properties this expression is aliased with using two way bindings
    pub two_way_bindings: Vec<NamedReference>,
}

impl std::convert::From<Expression> for BindingExpression {
    fn from(expression: Expression) -> Self {
        Self {
            expression,
            span: None,
            priority: 0,
            animation: Default::default(),
            analysis: Default::default(),
            two_way_bindings: Default::default(),
        }
    }
}

impl BindingExpression {
    pub fn new_uncompiled(node: SyntaxNode) -> Self {
        Self {
            expression: Expression::Uncompiled(node.clone()),
            span: Some(node.to_source_location()),
            priority: 1,
            animation: Default::default(),
            analysis: Default::default(),
            two_way_bindings: Default::default(),
        }
    }
    pub fn new_with_span(expression: Expression, span: SourceLocation) -> Self {
        Self {
            expression,
            span: Some(span),
            priority: 0,
            animation: Default::default(),
            analysis: Default::default(),
            two_way_bindings: Default::default(),
        }
    }

    /// Create an expression binding that simply is a two way binding to the other
    pub fn new_two_way(other: NamedReference) -> Self {
        Self {
            expression: Expression::Invalid,
            span: None,
            priority: 0,
            animation: Default::default(),
            analysis: Default::default(),
            two_way_bindings: vec![other],
        }
    }

    /// Merge the other into this one. Normally, &self is kept intact (has priority)
    /// unless the expression is invalid, in which case the other one is taken.
    ///
    /// Also the animation is taken if the other don't have one, and the two ways binding
    /// are taken into account.
    ///
    /// Returns true if the other expression was taken
    pub fn merge_with(&mut self, other: &Self) -> bool {
        if self.animation.is_none() {
            self.animation.clone_from(&other.animation);
        }
        let has_binding = self.has_binding();
        self.two_way_bindings.extend_from_slice(&other.two_way_bindings);
        if !has_binding {
            self.priority = other.priority;
            self.expression = other.expression.clone();
            true
        } else {
            false
        }
    }

    /// returns false if there is no expression or two way binding
    pub fn has_binding(&self) -> bool {
        !matches!(self.expression, Expression::Invalid) || !self.two_way_bindings.is_empty()
    }
}

impl Spanned for BindingExpression {
    fn span(&self) -> crate::diagnostics::Span {
        self.span.as_ref().map(|x| x.span()).unwrap_or_default()
    }
    fn source_file(&self) -> Option<&crate::diagnostics::SourceFile> {
        self.span.as_ref().and_then(|x| x.source_file())
    }
}

#[derive(Default, Debug, Clone)]
pub struct BindingAnalysis {
    /// true if that binding is part of a binding loop that already has been reported.
    pub is_in_binding_loop: Cell<bool>,

    /// true if the binding is a constant value that can be set without creating a binding at runtime
    pub is_const: bool,

    /// true if this binding does not depends on the value of property that are set externally.
    /// When true, this binding cannot be part of a binding loop involving external components
    pub no_external_dependencies: bool,
}

#[derive(Debug, Clone)]
pub enum Path {
    Elements(Vec<PathElement>),
    Events(Vec<Expression>, Vec<Expression>),
    Commands(Box<Expression>), // expr must evaluate to string
}

#[derive(Debug, Clone)]
pub struct PathElement {
    pub element_type: Rc<BuiltinElement>,
    pub bindings: BindingsMap,
}

#[derive(Clone, Debug, Default)]
pub enum EasingCurve {
    #[default]
    Linear,
    CubicBezier(f32, f32, f32, f32),
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,
    // CubicBezierNonConst([Box<Expression>; 4]),
    // Custom(Box<dyn Fn(f32)->f32>),
}

// The compiler generates ResourceReference::AbsolutePath for all references like @image-url("foo.png")
// and the resource lowering path may change this to EmbeddedData if configured.
#[derive(Clone, Debug)]
pub enum ImageReference {
    None,
    AbsolutePath(SmolStr),
    EmbeddedData { resource_id: usize, extension: String },
    EmbeddedTexture { resource_id: usize },
}

/// Print the expression as a .slint code (not necessarily valid .slint)
pub fn pretty_print(f: &mut dyn std::fmt::Write, expression: &Expression) -> std::fmt::Result {
    match expression {
        Expression::Invalid => write!(f, "<invalid>"),
        Expression::Uncompiled(u) => write!(f, "{u:?}"),
        Expression::StringLiteral(s) => write!(f, "{s:?}"),
        Expression::NumberLiteral(vl, unit) => write!(f, "{vl}{unit}"),
        Expression::BoolLiteral(b) => write!(f, "{b:?}"),
        Expression::PropertyReference(a) => write!(f, "{a:?}"),
        Expression::ElementReference(a) => write!(f, "{a:?}"),
        Expression::RepeaterIndexReference { element } => {
            crate::namedreference::pretty_print_element_ref(f, element)
        }
        Expression::RepeaterModelReference { element } => {
            crate::namedreference::pretty_print_element_ref(f, element)?;
            write!(f, ".@model")
        }
        Expression::FunctionParameterReference { index, ty: _ } => write!(f, "_arg_{index}"),
        Expression::StoreLocalVariable { name, value } => {
            write!(f, "{name} = ")?;
            pretty_print(f, value)
        }
        Expression::ReadLocalVariable { name, ty: _ } => write!(f, "{name}"),
        Expression::StructFieldAccess { base, name } => {
            pretty_print(f, base)?;
            write!(f, ".{name}")
        }
        Expression::ArrayIndex { array, index } => {
            pretty_print(f, array)?;
            write!(f, "[")?;
            pretty_print(f, index)?;
            write!(f, "]")
        }
        Expression::Cast { from, to } => {
            write!(f, "(")?;
            pretty_print(f, from)?;
            write!(f, "/* as {to} */)")
        }
        Expression::CodeBlock(c) => {
            write!(f, "{{ ")?;
            for e in c {
                pretty_print(f, e)?;
                write!(f, "; ")?;
            }
            write!(f, "}}")
        }
        Expression::FunctionCall { function, arguments, source_location: _ } => {
            match function {
                Callable::Builtin(b) => write!(f, "{b:?}")?,
                Callable::Callback(nr) | Callable::Function(nr) => write!(f, "{nr:?}")?,
            }
            write!(f, "(")?;
            for e in arguments {
                pretty_print(f, e)?;
                write!(f, ", ")?;
            }
            write!(f, ")")
        }
        Expression::SelfAssignment { lhs, rhs, op, .. } => {
            pretty_print(f, lhs)?;
            write!(f, " {}= ", if *op == '=' { ' ' } else { *op })?;
            pretty_print(f, rhs)
        }
        Expression::BinaryExpression { lhs, rhs, op } => {
            write!(f, "(")?;
            pretty_print(f, lhs)?;
            match *op {
                '=' | '!' => write!(f, " {op}= ")?,
                _ => write!(f, " {op} ")?,
            };
            pretty_print(f, rhs)?;
            write!(f, ")")
        }
        Expression::UnaryOp { sub, op } => {
            write!(f, "{op}")?;
            pretty_print(f, sub)
        }
        Expression::ImageReference { resource_ref, .. } => write!(f, "{resource_ref:?}"),
        Expression::Condition { condition, true_expr, false_expr } => {
            write!(f, "if (")?;
            pretty_print(f, condition)?;
            write!(f, ") {{ ")?;
            pretty_print(f, true_expr)?;
            write!(f, " }} else {{ ")?;
            pretty_print(f, false_expr)?;
            write!(f, " }}")
        }
        Expression::Array { element_ty: _, values } => {
            write!(f, "[")?;
            for e in values {
                pretty_print(f, e)?;
                write!(f, ", ")?;
            }
            write!(f, "]")
        }
        Expression::Struct { ty: _, values } => {
            write!(f, "{{ ")?;
            for (name, e) in values {
                write!(f, "{name}: ")?;
                pretty_print(f, e)?;
                write!(f, ", ")?;
            }
            write!(f, " }}")
        }
        Expression::PathData(data) => write!(f, "{data:?}"),
        Expression::EasingCurve(e) => write!(f, "{e:?}"),
        Expression::LinearGradient { angle, stops } => {
            write!(f, "@linear-gradient(")?;
            pretty_print(f, angle)?;
            for (c, s) in stops {
                write!(f, ", ")?;
                pretty_print(f, c)?;
                write!(f, "  ")?;
                pretty_print(f, s)?;
            }
            write!(f, ")")
        }
        Expression::RadialGradient { stops } => {
            write!(f, "@radial-gradient(circle")?;
            for (c, s) in stops {
                write!(f, ", ")?;
                pretty_print(f, c)?;
                write!(f, "  ")?;
                pretty_print(f, s)?;
            }
            write!(f, ")")
        }
        Expression::EnumerationValue(e) => match e.enumeration.values.get(e.value) {
            Some(val) => write!(f, "{}.{}", e.enumeration.name, val),
            None => write!(f, "{}.{}", e.enumeration.name, e.value),
        },
        Expression::ReturnStatement(e) => {
            write!(f, "return ")?;
            e.as_ref().map(|e| pretty_print(f, e)).unwrap_or(Ok(()))
        }
        Expression::LayoutCacheAccess { layout_cache_prop, index, repeater_index } => {
            write!(
                f,
                "{:?}[{}{}]",
                layout_cache_prop,
                index,
                if repeater_index.is_some() { " + $index" } else { "" }
            )
        }
        Expression::ComputeLayoutInfo(..) => write!(f, "layout_info(..)"),
        Expression::SolveLayout(..) => write!(f, "solve_layout(..)"),
        Expression::MinMax { ty: _, op, lhs, rhs } => {
            match op {
                MinMaxOp::Min => write!(f, "min(")?,
                MinMaxOp::Max => write!(f, "max(")?,
            }
            pretty_print(f, lhs)?;
            write!(f, ", ")?;
            pretty_print(f, rhs)?;
            write!(f, ")")
        }
        Expression::EmptyComponentFactory => write!(f, "<empty-component-factory>"),
        Expression::DebugHook { expression, id } => {
            write!(f, "debug-hook(")?;
            pretty_print(f, expression)?;
            write!(f, "\"{id}\")")
        }
    }
}
