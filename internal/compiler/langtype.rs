// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::collections::BTreeMap;
use std::fmt::Display;
use std::sync::Arc;

use smol_str::SmolStr;

use crate::diagnostics::SourceLocation;
use crate::expression_types::Unit;

#[derive(Debug, Clone, Default)]
pub enum Type {
    /// Correspond to an uninitialized type, or an error
    #[default]
    Invalid,
    /// The type of an expression that return nothing
    Void,
    /// The type of a property two way binding whose type was not yet inferred
    InferredProperty,
    /// The type of a callback alias whose type was not yet inferred
    InferredCallback,

    Callback(Arc<Function>),
    Function(Arc<Function>),

    ComponentFactory,

    // Other property types:
    Float32,
    Int32,
    String,
    Color,
    Duration,
    PhysicalLength,
    LogicalLength,
    Rem,
    Angle,
    Percent,
    Image,
    Bool,
    /// Fake type that can represent anything that can be converted into a model.
    Model,
    PathData, // Either a vector of path elements or a two vectors of events and coordinates
    Easing,
    Brush,
    /// This is usually a model
    Array(Arc<Type>),
    Struct(Arc<Struct>),
    Enumeration(Arc<Enumeration>),
    Keys,
    /// `data-transfer` - a special type that handles reading a value from the system with
    /// some set of available MIME types.
    DataTransfer,

    /// A type made up of the product of several "unit" types.
    /// The first parameter is the unit, and the second parameter is the power.
    /// The vector should be sorted by 1) the power, 2) the unit.
    UnitProduct(Vec<(Unit, i8)>),

    ElementReference,

    /// This is a `SharedVector<Coord>`
    ArrayOfCoord,
    /// This is used by GridLayoutOrganizedData
    ArrayOfU16,

    StyledText,
    MouseCursor,
    Closure,
}

impl core::cmp::PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        match self {
            Type::Invalid => matches!(other, Type::Invalid),
            Type::Void => matches!(other, Type::Void),
            Type::InferredProperty => matches!(other, Type::InferredProperty),
            Type::InferredCallback => matches!(other, Type::InferredCallback),
            Type::Callback(lhs) => {
                matches!(other, Type::Callback(rhs) if lhs == rhs)
            }
            Type::Function(lhs) => {
                matches!(other, Type::Function(rhs) if lhs == rhs)
            }
            Type::ComponentFactory => matches!(other, Type::ComponentFactory),
            Type::Float32 => matches!(other, Type::Float32),
            Type::Int32 => matches!(other, Type::Int32),
            Type::String => matches!(other, Type::String),
            Type::Color => matches!(other, Type::Color),
            Type::Duration => matches!(other, Type::Duration),
            Type::Angle => matches!(other, Type::Angle),
            Type::PhysicalLength => matches!(other, Type::PhysicalLength),
            Type::LogicalLength => matches!(other, Type::LogicalLength),
            Type::Rem => matches!(other, Type::Rem),
            Type::Percent => matches!(other, Type::Percent),
            Type::Image => matches!(other, Type::Image),
            Type::Bool => matches!(other, Type::Bool),
            Type::Model => matches!(other, Type::Model),
            Type::PathData => matches!(other, Type::PathData),
            Type::Easing => matches!(other, Type::Easing),
            Type::MouseCursor => matches!(other, Type::MouseCursor),
            Type::Brush => matches!(other, Type::Brush),
            Type::Array(a) => matches!(other, Type::Array(b) if a == b),
            Type::Struct(lhs) => {
                matches!(other, Type::Struct(rhs) if lhs.fields == rhs.fields && lhs.name == rhs.name)
            }
            Type::Enumeration(lhs) => matches!(other, Type::Enumeration(rhs) if lhs == rhs),
            Type::Keys => matches!(other, Type::Keys),
            Type::UnitProduct(a) => matches!(other, Type::UnitProduct(b) if a == b),
            Type::ElementReference => matches!(other, Type::ElementReference),
            Type::ArrayOfCoord => matches!(other, Type::ArrayOfCoord),
            Type::ArrayOfU16 => matches!(other, Type::ArrayOfU16),
            Type::StyledText => matches!(other, Type::StyledText),
            Type::DataTransfer => matches!(other, Type::DataTransfer),
            Type::Closure => matches!(other, Type::Closure),
        }
    }
}

impl Display for Type {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Type::Invalid => write!(f, "<error>"),
            Type::Void => write!(f, "void"),
            Type::InferredProperty => write!(f, "?"),
            Type::InferredCallback => write!(f, "callback"),
            Type::Callback(callback) => {
                write!(f, "callback{}", callback)
            }
            Type::ComponentFactory => write!(f, "component-factory"),
            Type::Function(function) => {
                write!(f, "function{}", function)
            }
            Type::Float32 => write!(f, "float"),
            Type::Int32 => write!(f, "int"),
            Type::String => write!(f, "string"),
            Type::Duration => write!(f, "duration"),
            Type::Angle => write!(f, "angle"),
            Type::PhysicalLength => write!(f, "physical-length"),
            Type::LogicalLength => write!(f, "length"),
            Type::Rem => write!(f, "relative-font-size"),
            Type::Percent => write!(f, "percent"),
            Type::Color => write!(f, "color"),
            Type::Image => write!(f, "image"),
            Type::Bool => write!(f, "bool"),
            Type::Model => write!(f, "model"),
            Type::Array(t) => write!(f, "[{t}]"),
            Type::Struct(t) => write!(f, "{t}"),
            Type::PathData => write!(f, "pathdata"),
            Type::Easing => write!(f, "easing"),
            Type::MouseCursor => write!(f, "MouseCursor"),
            Type::Brush => write!(f, "brush"),
            Type::Enumeration(enumeration) => write!(f, "{}", enumeration.name),
            Type::Keys => write!(f, "keys"),
            Type::DataTransfer => write!(f, "data-transfer"),
            Type::UnitProduct(vec) => {
                const POWERS: &[char] = &['⁰', '¹', '²', '³', '⁴', '⁵', '⁶', '⁷', '⁸', '⁹'];
                f.write_str("(")?;
                for (i, (unit, power)) in vec.iter().enumerate() {
                    if i > 0 {
                        f.write_str("×")?;
                    }
                    write!(f, "{unit}")?;
                    if *power == 1 {
                        continue;
                    }
                    if *power < 0 {
                        f.write_str("⁻")?;
                    }
                    for x in power.unsigned_abs().to_string().as_bytes() {
                        write!(f, "{}", POWERS[(x - b'0') as usize])?;
                    }
                }
                f.write_str(")")
            }
            Type::ElementReference => write!(f, "element ref"),
            Type::ArrayOfCoord => write!(f, "[coord]"),
            Type::ArrayOfU16 => write!(f, "[u16]"),
            Type::StyledText => write!(f, "styled-text"),
            Type::Closure => write!(f, "closure"),
        }
    }
}

impl From<Arc<Struct>> for Type {
    fn from(value: Arc<Struct>) -> Self {
        Self::Struct(value)
    }
}

impl Type {
    /// Whether the type is part of the Slint SC subset.
    /// Callable without the `slint-sc` feature, so shared call sites need no `cfg`.
    pub fn is_slint_sc(&self) -> bool {
        #[cfg(feature = "slint-sc")]
        return match self {
            Self::Int32 | Self::LogicalLength | Self::Color | Self::Bool | Self::Image => true,
            // A user-declared enum.
            Self::Enumeration(en) => en.node.is_some(),
            // A user-declared struct. Its field types were validated where the
            // struct was declared, so they need no re-check here.
            Self::Struct(s) => matches!(&s.name, StructName::User { .. }),
            _ => false,
        };
        #[cfg(not(feature = "slint-sc"))]
        false
    }

    /// The experimental feature that must be enabled to use this type
    pub fn experimental_feature(&self) -> Option<crate::ExperimentalFeature> {
        matches!(self, Type::ComponentFactory)
            .then_some(crate::ExperimentalFeature::ComponentContainer)
    }

    /// valid type for properties
    pub fn is_property_type(&self) -> bool {
        matches!(
            self,
            Self::Float32
                | Self::Int32
                | Self::String
                | Self::Color
                | Self::ComponentFactory
                | Self::Duration
                | Self::Angle
                | Self::PhysicalLength
                | Self::LogicalLength
                | Self::Rem
                | Self::Percent
                | Self::Image
                | Self::Bool
                | Self::Easing
                | Self::MouseCursor
                | Self::Enumeration(_)
                | Self::Keys
                | Self::DataTransfer
                | Self::ElementReference
                | Self::Struct { .. }
                | Self::Array(_)
                | Self::Brush
                | Self::InferredProperty
                | Self::StyledText
        )
    }

    pub fn ok_for_public_api(&self) -> bool {
        !matches!(self, Self::Easing)
    }

    /// Assume it is an enumeration, panic if it isn't
    pub fn as_enum(&self) -> &Arc<Enumeration> {
        match self {
            Type::Enumeration(e) => e,
            _ => panic!("should be an enumeration, bug in compiler pass"),
        }
    }

    /// Return true if the type can be converted to the other type
    pub fn can_convert(&self, other: &Self) -> bool {
        let can_convert_struct = |a: &BTreeMap<SmolStr, Type>, b: &BTreeMap<SmolStr, Type>| {
            // the struct `b` has property that the struct `a` doesn't
            let mut has_more_property = false;
            for (k, v) in b {
                match a.get(k) {
                    Some(t) if !t.can_convert(v) => return false,
                    None => has_more_property = true,
                    _ => (),
                }
            }
            if has_more_property {
                // we should reject the conversion if `a` has property that `b` doesn't have
                if a.keys().any(|k| !b.contains_key(k)) {
                    return false;
                }
            }
            true
        };
        match (self, other) {
            (a, b) if a == b => true,
            (_, Type::Invalid)
            | (_, Type::Void)
            | (Type::Float32, Type::Int32)
            | (Type::Float32, Type::String)
            | (Type::Int32, Type::Float32)
            | (Type::Int32, Type::String)
            | (Type::Float32, Type::Model)
            | (Type::Int32, Type::Model)
            | (Type::PhysicalLength, Type::LogicalLength)
            | (Type::LogicalLength, Type::PhysicalLength)
            | (Type::Rem, Type::LogicalLength)
            | (Type::Rem, Type::PhysicalLength)
            | (Type::LogicalLength, Type::Rem)
            | (Type::PhysicalLength, Type::Rem)
            | (Type::Percent, Type::Float32)
            | (Type::Brush, Type::Color)
            | (Type::Color, Type::Brush) => true,
            (Type::Array(a), Type::Model) if a.is_property_type() => true,
            (Type::Struct(a), Type::Struct(b)) => can_convert_struct(&a.fields, &b.fields),
            (Type::UnitProduct(u), o) => match o.as_unit_product() {
                Some(o) => unit_product_length_conversion(u.as_slice(), o.as_slice()).is_some(),
                None => false,
            },
            (o, Type::UnitProduct(u)) => match o.as_unit_product() {
                Some(o) => unit_product_length_conversion(u.as_slice(), o.as_slice()).is_some(),
                None => false,
            },
            _ => false,
        }
    }

    /// If this is a number type which should be used with an unit, this returns the default unit
    /// otherwise, returns None
    pub fn default_unit(&self) -> Option<Unit> {
        match self {
            Type::Duration => Some(Unit::Ms),
            Type::PhysicalLength => Some(Unit::Phx),
            Type::LogicalLength => Some(Unit::Px),
            Type::Rem => Some(Unit::Rem),
            // Unit::Percent is special that it does not combine with other units like
            Type::Percent => None,
            Type::Angle => Some(Unit::Deg),
            Type::Invalid => None,
            Type::Void => None,
            Type::InferredProperty | Type::InferredCallback => None,
            Type::Callback { .. } => None,
            Type::ComponentFactory => None,
            Type::Function { .. } => None,
            Type::Float32 => None,
            Type::Int32 => None,
            Type::String => None,
            Type::Color => None,
            Type::Image => None,
            Type::Bool => None,
            Type::Model => None,
            Type::PathData => None,
            Type::Easing => None,
            Type::MouseCursor => None,
            Type::Brush => None,
            Type::Array(_) => None,
            Type::Struct { .. } => None,
            Type::Enumeration(_) => None,
            Type::Keys => None,
            Type::DataTransfer => None,
            Type::UnitProduct(_) => None,
            Type::ElementReference => None,
            Type::ArrayOfCoord => None,
            Type::ArrayOfU16 => None,
            Type::StyledText => None,
            Type::Closure => None,
        }
    }

    /// Return a unit product vector even for single scalar
    pub fn as_unit_product(&self) -> Option<Vec<(Unit, i8)>> {
        match self {
            Type::UnitProduct(u) => Some(u.clone()),
            Type::Float32 | Type::Int32 => Some(Vec::new()),
            Type::Percent => Some(Vec::new()),
            _ => self.default_unit().map(|u| vec![(u, 1)]),
        }
    }
}

macro_rules! define_builtin_struct_enum {
    ($(
        $(#[$attr:meta])*
        $vis:vis struct $Name:ident {
            $( $(#[$field_attr:meta])* $field:ident : $field_type:ty $(= $field_default:expr)?, )*
        }
    )*) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, strum::EnumString, strum::IntoStaticStr)]
        pub enum BuiltinStruct {
            // Generated from for_each_builtin_structs
            $($Name,)*

            // Public structs not in the macro (registered in typeregister.rs)
            LogicalPosition,
            LogicalSize,

            // The results of a color's `to-hsv()` and `to-oklch()`, and the struct that its
            // `red`, `green`, `blue` and `alpha` come from. They have no name in Slint.
            ColorRgba,
            ColorHsva,
            ColorOklch,

            // Path element types, set via the `builtin_struct` flag of the
            // builtin element declaration and read through NativeClass.builtin_struct
            PathMoveTo,
            PathLineTo,
            PathArcTo,
            PathCubicTo,
            PathQuadraticTo,
            PathClose,
            PathElement,

            // Compiler-internal structs (no slint_name, not exposed to .slint)

            // Internal coordinate struct for compiled SVG path data (x/y as Float32,
            // unlike LogicalPosition which uses LogicalLength)
            Point,
            // Return type of ArraySize (width/height as Int32,
            // unlike LogicalSize which uses LogicalLength)
            Size,
            StateInfo,
            PropertyAnimation,
            GridLayoutData,
            GridLayoutInputData,
            BoxLayoutData,
            BoxLayoutOrthoData,
            FlexboxLayoutData,
            LayoutItemInfo,
            FlexboxLayoutItemInfo,
            FlexItemProps,
            Padding,
            LayoutInfo,
        }

        impl BuiltinStruct {
            pub fn is_public(&self) -> bool {
                match self {
                    // Macro-defined structs: derived from the `pub` visibility keyword
                    $(Self::$Name => stringify!($vis) == "pub",)*
                    // Non-macro public structs
                    Self::LogicalPosition | Self::LogicalSize => true,
                    _ => false,
                }
            }

            /// The name of this struct in the Slint language, or None if it is
            /// purely internal and not visible in .slint files.
            pub fn slint_name(&self) -> Option<SmolStr> {
                match self {
                    // Macro-defined structs all have a slint name matching their Rust name
                    $(Self::$Name => {
                        Some(SmolStr::new_static(stringify!($Name)))
                    })*
                    // Non-macro structs with custom slint names
                    Self::LogicalPosition => Some(SmolStr::new_static("Point")),
                    Self::LogicalSize => Some(SmolStr::new_static("Size")),
                    _ => None,
                }
            }
        }
    };
}
i_slint_common::for_each_builtin_structs!(define_builtin_struct_enum);

impl BuiltinStruct {
    pub fn is_layout_data(&self) -> bool {
        matches!(
            self,
            Self::GridLayoutInputData
                | Self::GridLayoutData
                | Self::BoxLayoutData
                | Self::BoxLayoutOrthoData
                | Self::FlexboxLayoutData
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub return_type: Type,
    pub args: Vec<Type>,
    /// The optional names of the arguments (empty string means not set).
    /// The names are not technically part of the type, but it is good to have them available for auto-completion
    pub arg_names: Vec<SmolStr>,
}

impl Display for Function {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "(")?;
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 {
                write!(formatter, ", ")?;
            }
            match self.arg_names.get(i).filter(|name| !name.is_empty()) {
                Some(name) => write!(formatter, "{name}: {arg}")?,
                None => write!(formatter, "{arg}")?,
            }
        }
        let return_type = if self.return_type == Type::Void {
            String::new()
        } else {
            format!(" -> {}", self.return_type)
        };
        write!(formatter, "){return_type}")
    }
}

#[derive(Debug, Clone)]
pub enum StructName {
    /// Anonymous structs
    None,
    /// When declared in .slint as  `struct Foo { }`, then the name is "Foo"
    User {
        name: SmolStr,
        /// Where the declaration was written (for the language server).
        node: SourceLocation,
        /// The raw text of each `@rust-attr(...)` on the declaration, captured
        /// at build time so the Rust generator does not need the syntax tree.
        rust_attributes: Vec<SmolStr>,
        /// The field names in declaration order. The C++ generator emits struct
        /// members in this order (positional aggregate initialization relies on
        /// it), which `fields` — a sorted map — does not preserve.
        field_order: Vec<SmolStr>,
    },
    Builtin(BuiltinStruct),
}

impl PartialEq for StructName {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::User { name: l_user_name, .. }, Self::User { name: r_user_name, .. }) => {
                l_user_name == r_user_name
            }
            (Self::Builtin(l0), Self::Builtin(r0)) => l0 == r0,
            _ => core::mem::discriminant(self) == core::mem::discriminant(other),
        }
    }
}

impl StructName {
    pub fn slint_name(&self) -> Option<SmolStr> {
        match self {
            StructName::None => None,
            StructName::User { name, .. } => Some(name.clone()),
            StructName::Builtin(builtin) => builtin.slint_name(),
        }
    }

    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }

    pub fn is_some(&self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn or(self, other: Self) -> Self {
        match self {
            Self::None => other,
            this => this,
        }
    }
}

impl From<BuiltinStruct> for StructName {
    fn from(value: BuiltinStruct) -> Self {
        Self::Builtin(value)
    }
}

#[derive(Debug, Clone)]
pub struct Struct {
    pub fields: BTreeMap<SmolStr, Type>,
    /// Default values for the fields.
    /// The expressions are resolved, converted to the field type, and constant-folded.
    /// Like the syntax node in `name`, this is ignored by `Type`'s equality comparison.
    pub field_defaults: BTreeMap<SmolStr, ConstantExpression>,
    pub name: StructName,
}

impl Struct {
    /// Create a struct without declared field default values.
    pub fn new(fields: BTreeMap<SmolStr, Type>, name: impl Into<StructName>) -> Self {
        Self { fields, field_defaults: Default::default(), name: name.into() }
    }

    /// Where a user-declared struct was written (for the language server).
    pub fn node(&self) -> Option<&SourceLocation> {
        match &self.name {
            StructName::User { node, .. } => Some(node),
            _ => None,
        }
    }

    /// The raw text of each `@rust-attr(...)` on a user-declared struct.
    pub fn rust_attributes(&self) -> &[SmolStr] {
        match &self.name {
            StructName::User { rust_attributes, .. } => rust_attributes,
            _ => &[],
        }
    }

    /// The field names in declaration order for a user-declared struct.
    pub fn field_order(&self) -> &[SmolStr] {
        match &self.name {
            StructName::User { field_order, .. } => field_order,
            _ => &[],
        }
    }
}

/// A constant expression, used for the default values of struct fields
/// (see [`Struct::field_defaults`]).
///
/// This is deliberately neither [`Expression`](crate::expression_tree::Expression) nor an llr expression:
/// unlike those, it cannot reference any properties, elements, or syntax nodes,
/// so a [`Struct`] carrying one can safely outlive the object tree.
/// The variants are the subset that every consumer can materialize.
/// Keep the matches over this type exhaustive,
/// so that adding a variant is a compile error in each consumer:
/// the conversion to an expression tree (`Expression::from_constant`),
/// the lowering for the code generators (`lower_constant_expression` in the llr module),
/// and the interpreter's evaluator (`eval_constant_expression` there).
#[derive(Debug, Clone)]
pub enum ConstantExpression {
    StringLiteral(SmolStr),
    /// A number and its unit, in normalized form
    NumberLiteral(f64, Unit),
    BoolLiteral(bool),
    EnumerationValue(EnumerationValue),
    Cast {
        from: Box<ConstantExpression>,
        to: Type,
    },
    UnaryOp {
        sub: Box<ConstantExpression>,
        op: char,
    },
    Struct {
        ty: Arc<Struct>,
        values: BTreeMap<SmolStr, ConstantExpression>,
    },
    Array {
        element_ty: Type,
        values: Vec<ConstantExpression>,
    },
}

impl Display for Struct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(name) = &self.name.slint_name() {
            write!(f, "{name}")
        } else {
            write!(f, "{{ ")?;
            for (k, v) in &self.fields {
                write!(f, "{k}: {v},")?;
            }
            write!(f, "}}")
        }
    }
}

/// Call `visitor` for every user-declared (non-builtin) struct or enum reachable from `ty`,
/// recursing through struct fields, arrays, and callback/function signatures.
pub(crate) fn visit_declared_types(ty: &Type, visitor: &mut impl FnMut(&SmolStr, &Type)) {
    match ty {
        Type::Struct(s) => {
            if let StructName::User { name, .. } = &s.name {
                visitor(name, ty);
            }
            for sub_ty in s.fields.values() {
                visit_declared_types(sub_ty, visitor);
            }
        }
        Type::Array(x) => visit_declared_types(x, visitor),
        Type::Function(function) | Type::Callback(function) => {
            visit_declared_types(&function.return_type, visitor);
            for a in &function.args {
                visit_declared_types(a, visitor);
            }
        }
        Type::Enumeration(en) if en.node.is_some() => visitor(&en.name, ty),
        _ => {}
    }
}

#[derive(Debug, Clone)]
pub struct Enumeration {
    pub name: SmolStr,
    pub values: Vec<SmolStr>,
    pub default_value: usize, // index in values
    // For non-builtins enums, this is where the declaration was written.
    pub node: Option<SourceLocation>,
    /// Whether a built-in enum is re-exported as `slint::language::X`.
    pub public: bool,
    /// The raw text of each `@rust-attr(...)` on the declaration, captured at
    /// build time so the Rust generator does not need the syntax tree.
    pub rust_attributes: Vec<SmolStr>,
}

impl PartialEq for Enumeration {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
    }
}

impl Enumeration {
    pub fn default_value(self: Arc<Self>) -> EnumerationValue {
        EnumerationValue { value: self.default_value, enumeration: self.clone() }
    }

    pub fn try_value_from_string(self: Arc<Self>, value: &str) -> Option<EnumerationValue> {
        self.values.iter().enumerate().find_map(|(idx, name)| {
            if name == value {
                Some(EnumerationValue { value: idx, enumeration: self.clone() })
            } else {
                None
            }
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct KeyboardModifiers {
    pub alt: bool,
    pub control: bool,
    pub meta: bool,
    pub shift: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct Keys {
    pub key: SmolStr,
    pub modifiers: KeyboardModifiers,
    pub ignore_shift: bool,
    pub ignore_alt: bool,
}

impl std::fmt::Display for Keys {
    // Make sure to keep this in sync with the implementation in core/input.rs
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.key.is_empty() {
            write!(f, "")
        } else {
            let alt = self
                .ignore_alt
                .then_some("Alt?+")
                .or(self.modifiers.alt.then_some("Alt+"))
                .unwrap_or_default();
            let ctrl = if self.modifiers.control { "Control+" } else { "" };
            let meta = if self.modifiers.meta { "Meta+" } else { "" };
            let shift = self
                .ignore_shift
                .then_some("Shift?+")
                .or(self.modifiers.shift.then_some("Shift+"))
                .unwrap_or_default();
            let keycode: String = self
                .key
                .chars()
                .flat_map(|character| {
                    let mut escaped = vec![];
                    if character.is_control() {
                        escaped.extend(character.escape_unicode());
                    } else {
                        escaped.push(character);
                    }
                    escaped
                })
                .collect();
            write!(f, "{meta}{ctrl}{alt}{shift}\"{keycode}\"")
        }
    }
}

#[derive(Clone, Debug)]
pub struct EnumerationValue {
    pub value: usize, // index in enumeration.values
    pub enumeration: Arc<Enumeration>,
}

impl PartialEq for EnumerationValue {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value && same_declaration(&self.enumeration, &other.enumeration)
    }
}

/// Whether two enums come from the same declaration, including a copy renamed by a pass.
/// Two declarations in different files can share a name.
fn same_declaration(a: &Arc<Enumeration>, b: &Arc<Enumeration>) -> bool {
    Arc::ptr_eq(a, b)
        || matches!((&a.node, &b.node), (Some(a), Some(b))
            if a.span.offset == b.span.offset
                && a.source_file.as_ref().map(|f| f.path()) == b.source_file.as_ref().map(|f| f.path()))
}

impl Eq for EnumerationValue {}

impl std::hash::Hash for EnumerationValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match &self.enumeration.node {
            Some(node) => node.span.offset.hash(state),
            None => Arc::as_ptr(&self.enumeration).hash(state),
        }
        self.value.hash(state);
    }
}

impl std::fmt::Display for EnumerationValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.enumeration.values[self.value].fmt(f)
    }
}

impl EnumerationValue {
    pub fn to_pascal_case(&self) -> String {
        crate::names::to_pascal_case(&self.enumeration.values[self.value])
    }
}

#[derive(Debug, PartialEq)]
pub struct LengthConversionPowers {
    pub rem_to_px_power: i8,
    pub px_to_phx_power: i8,
}

/// If the `Type::UnitProduct(a)` can be converted to `Type::UnitProduct(b)` by multiplying
/// by the scale factor, return that scale factor, otherwise, return None
pub fn unit_product_length_conversion(
    a: &[(Unit, i8)],
    b: &[(Unit, i8)],
) -> Option<LengthConversionPowers> {
    // e.g. float to int conversion, no units
    if a.is_empty() && b.is_empty() {
        return Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: 0 });
    }

    let mut units = [0i8; 16];
    for (u, count) in a {
        units[*u as usize] += count;
    }
    for (u, count) in b {
        units[*u as usize] -= count;
    }

    if units[Unit::Px as usize] + units[Unit::Phx as usize] + units[Unit::Rem as usize] != 0 {
        return None;
    }

    if units[Unit::Rem as usize] != 0
        && units[Unit::Phx as usize] == -units[Unit::Rem as usize]
        && units[Unit::Px as usize] == 0
    {
        units[Unit::Px as usize] = -units[Unit::Rem as usize];
        units[Unit::Phx as usize] = -units[Unit::Rem as usize];
    }

    let result = LengthConversionPowers {
        rem_to_px_power: if units[Unit::Rem as usize] != 0 { units[Unit::Px as usize] } else { 0 },
        px_to_phx_power: if units[Unit::Px as usize] != 0 { units[Unit::Phx as usize] } else { 0 },
    };

    units[Unit::Px as usize] = 0;
    units[Unit::Phx as usize] = 0;
    units[Unit::Rem as usize] = 0;
    units.into_iter().all(|x| x == 0).then_some(result)
}

#[test]
fn unit_product_length_conversion_test() {
    use Option::None;
    use Unit::*;
    assert_eq!(
        unit_product_length_conversion(&[], &[]),
        Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: 0 })
    );
    assert_eq!(
        unit_product_length_conversion(&[(Px, 1)], &[(Phx, 1)]),
        Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: -1 })
    );
    assert_eq!(
        unit_product_length_conversion(&[(Phx, -2)], &[(Px, -2)]),
        Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: -2 })
    );
    assert_eq!(
        unit_product_length_conversion(&[(Px, 1), (Phx, -2)], &[(Phx, -1)]),
        Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: -1 })
    );
    assert_eq!(
        unit_product_length_conversion(
            &[(Deg, 3), (Phx, 2), (Ms, -1)],
            &[(Phx, 4), (Deg, 3), (Ms, -1), (Px, -2)]
        ),
        Some(LengthConversionPowers { rem_to_px_power: 0, px_to_phx_power: -2 })
    );
    assert_eq!(unit_product_length_conversion(&[(Px, 1)], &[(Phx, -1)]), None);
    assert_eq!(unit_product_length_conversion(&[(Deg, 1), (Phx, -2)], &[(Px, -2)]), None);
    assert_eq!(unit_product_length_conversion(&[(Px, 1)], &[(Phx, -1)]), None);

    assert_eq!(
        unit_product_length_conversion(&[(Rem, 1)], &[(Px, 1)]),
        Some(LengthConversionPowers { rem_to_px_power: -1, px_to_phx_power: 0 })
    );
    assert_eq!(
        unit_product_length_conversion(&[(Rem, 1)], &[(Phx, 1)]),
        Some(LengthConversionPowers { rem_to_px_power: -1, px_to_phx_power: -1 })
    );
    assert_eq!(
        unit_product_length_conversion(&[(Rem, 2)], &[(Phx, 2)]),
        Some(LengthConversionPowers { rem_to_px_power: -2, px_to_phx_power: -2 })
    );
}
