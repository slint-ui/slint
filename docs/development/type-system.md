# Slint Type System

> Note for AI coding assistants (agents):
> **When to load this document:** Working on `internal/compiler/langtype.rs`,
> `internal/compiler/lookup.rs`, `internal/compiler/typeregister.rs`,
> type checking passes, or debugging type inference issues.
> For general build commands and project structure, see `/AGENTS.md`.

## Overview

Slint has a rich type system that includes primitive types, unit types for dimensional quantities, composite types (structs, enumerations), callbacks, functions, and element types. The type system supports:

- **Unit types** for compile-time dimension checking (px, phx, rem, ms, deg, %)
- **Automatic conversions** between compatible types
- **Type inference** for property bindings and two-way bindings
- **Generic element types** for components and built-in items

## Key Files

| File | Purpose |
|------|---------|
| `internal/compiler/langtype.rs` | Core `Type` enum and type definitions |
| `internal/compiler/lookup.rs` | Name resolution and expression lookup |
| `internal/compiler/typeregister.rs` | Type registry, built-in types, reserved properties |
| `internal/compiler/expression_tree.rs` | Unit definitions and expressions |
| `internal/compiler/typeloader.rs` | Import resolution and document loading |

## Core Type Enum

The `Type` enum represents all possible types in Slint:

```rust
pub enum Type {
    // Error/placeholder types
    Invalid,           // Uninitialized or error
    Void,              // Expression returns nothing
    InferredProperty,  // Two-way binding type not yet inferred
    InferredCallback,  // Callback alias type not yet inferred

    // Callable types
    Callback(Rc<Function>),
    Function(Rc<Function>),

    // Primitive types
    Float32,
    Int32,
    String,
    Bool,

    // Unit types (dimensional quantities)
    Duration,          // Time (ms, s)
    PhysicalLength,    // Physical pixels (phx)
    LogicalLength,     // Logical pixels (px, cm, mm, in, pt)
    Rem,               // Font-relative size
    Angle,             // Rotation (deg, rad, turn, grad)
    Percent,           // Percentage values

    // Visual types
    Color,
    Brush,
    Image,
    Easing,

    // Composite types
    Array(Rc<Type>),
    Struct(Rc<Struct>),
    Enumeration(Rc<Enumeration>),

    // Special types
    Model,             // Anything convertible to a model
    UnitProduct(Vec<(Unit, i8)>),  // Product of units (e.g., px²)
    ElementReference,  // Reference to an element
    ComponentFactory,  // Factory for dynamic components
    // ... internal types
}
```

## Unit System

Units provide compile-time dimension checking. A number with a unit becomes a typed value:

### Available Units

| Unit | Syntax | Type | Notes |
|------|--------|------|-------|
| None | `100` | `Float32` | Unitless number |
| Percent | `50%` | `Percent` | Percentage |
| Phx | `100phx` | `PhysicalLength` | Physical pixels |
| Px | `100px` | `LogicalLength` | Logical pixels |
| Cm | `2.5cm` | `LogicalLength` | Centimeters (×37.8) |
| Mm | `25mm` | `LogicalLength` | Millimeters (×3.78) |
| In | `1in` | `LogicalLength` | Inches (×96) |
| Pt | `12pt` | `LogicalLength` | Points (×96/72) |
| Rem | `1.5rem` | `Rem` | Font-relative size |
| S | `2s` | `Duration` | Seconds (×1000) |
| Ms | `500ms` | `Duration` | Milliseconds |
| Deg | `45deg` | `Angle` | Degrees |
| Grad | `50grad` | `Angle` | Gradians |
| Turn | `0.25turn` | `Angle` | Turns (×360) |
| Rad | `3.14rad` | `Angle` | Radians |

### Unit Products

For expressions like `width * height`, the type system tracks unit products:

```rust
// Type::UnitProduct(vec![(Unit::Px, 2)])  represents px²
// This allows: area: length * length; // Valid
// And catches: area: length + length; // Type mismatch
```

The `unit_product_length_conversion()` function determines if one unit product can be converted to another by multiplying by scale factors (px↔phx conversion, rem↔px conversion).

## Type Conversions

The `can_convert()` method defines which types can be implicitly converted:

### Allowed Conversions

```
Float32 ↔ Int32          (numeric conversion)
Float32 → String         (to_string)
Int32 → String           (to_string)
Float32/Int32 → Model    (single-element model)
PhysicalLength ↔ LogicalLength  (scale factor)
Rem ↔ LogicalLength      (font-size multiplication)
Rem ↔ PhysicalLength     (combined conversion)
Percent → Float32        (divide by 100)
Color ↔ Brush            (solid brush)
Array<T> → Model         (where T is property type)
Struct → Struct          (compatible fields)
```

### Struct Compatibility

Struct A can convert to Struct B if:
1. All fields in B exist in A with convertible types
2. If B has extra fields, A must not have any fields missing from B

```slint,ignore
// This works:
struct Small { x: int }
struct Large { x: int, y: int }
property<Large> p: { x: 5 };  // OK: y gets default value
```

## Element Types

Elements (components/items) have their own type hierarchy:

```rust
pub enum ElementType {
    Component(Rc<Component>),  // User-defined component
    Builtin(Rc<BuiltinElement>),  // Built-in item (Rectangle, Text, etc.)
    Native(Rc<NativeClass>),   // After native class resolution
    Error,                     // Lookup failed
    Global,                    // Global component base
    Interface,                 // Interface base
}
```

### Property Lookup on Elements

When looking up a property on an element:

1. Check the element's declared properties
2. Check inherited properties from base type
3. For built-in elements, check `BuiltinElement.properties`
4. For item types, check reserved properties (x, y, width, height, etc.)
5. Handle property aliases (deprecated names)

```rust
impl ElementType {
    pub fn lookup_property(&self, name: &str) -> PropertyLookupResult {
        // Returns type, visibility, deprecated status, etc.
    }
}
```

## Name Resolution (Lookup)

The `LookupCtx` provides context for resolving identifiers in expressions:

```rust
pub struct LookupCtx<'a> {
    pub property_name: Option<&'a str>,     // Current property being bound
    pub property_type: Type,                 // Expected type
    pub component_scope: &'a [ElementRc],   // Element scope stack
    pub arguments: Vec<SmolStr>,             // Callback/function arguments
    pub type_register: &'a TypeRegister,    // Type registry
    pub local_variables: Vec<Vec<(SmolStr, Type)>>,  // Local variable scopes
}
```

### Lookup Order

When resolving an identifier, lookup proceeds in this order:

1. **Local variables** - Variables declared in the current scope
2. **Arguments** - Callback/function parameters
3. **Special identifiers** - `self`, `parent`, `true`, `false`
4. **Element IDs** - Named elements in the component
5. **In-scope properties** - Properties from scope stack (legacy syntax: parent properties)
6. **Built-in namespaces** - `Colors`, `Math`, `Key`, `Easing`
7. **Global types** - Types from the type register

### LookupResult

Lookup returns one of:

```rust
pub enum LookupResult {
    Expression { expression: Expression, deprecated: Option<String> },
    Enumeration(Rc<Enumeration>),
    Namespace(BuiltinNamespace),
    Callable(LookupResultCallable),
}
```

## Type Register

The `TypeRegister` maintains all known types:

```rust
pub struct TypeRegister {
    types: HashMap<SmolStr, Type>,
    elements: HashMap<SmolStr, ElementType>,
    pub expose_internal_types: bool,
    // ...
}
```

### Built-in Types

The register is initialized with:

1. **Primitive types**: `int`, `float`, `string`, `bool`, `color`, etc.
2. **Built-in enumerations**: `TextHorizontalAlignment`, `ImageFit`, etc.
3. **Built-in structs**: `Point`, `KeyEvent`, `PointerEvent`, etc.
4. **Built-in elements**: `Rectangle`, `Text`, `Image`, etc.

### Reserved Properties

All items automatically get reserved properties:

```rust
// Geometry
("x", Type::LogicalLength),
("y", Type::LogicalLength),
("width", Type::LogicalLength),
("height", Type::LogicalLength),

// Layout
("min-width", Type::LogicalLength),
("max-width", Type::LogicalLength),
("preferred-width", Type::LogicalLength),
("horizontal-stretch", Type::Float32),
// ...

// Grid layout
("col", Type::Int32),
("row", Type::Int32),
("colspan", Type::Int32),
("rowspan", Type::Int32),

// Accessibility
("accessible-role", AccessibleRole),
("accessible-label", Type::String),
// ...
```

## Property Visibility

Properties have visibility levels that control access:

```rust
pub enum PropertyVisibility {
    Private,    // Only accessible within the component
    Input,      // Can be set from outside, read inside
    Output,     // Can be read from outside, set inside
    InOut,      // Both readable and writable
    Public,     // For functions/callbacks
    Constexpr,  // Compile-time constant
}
```

### Visibility Rules

| Visibility | Set from outside | Set from inside | Read from outside | Read from inside |
|------------|-----------------|-----------------|-------------------|------------------|
| Private    | No | Yes | No | Yes |
| Input      | Yes | No | No | Yes |
| Output     | No | Yes | Yes | Yes |
| InOut      | Yes | Yes | Yes | Yes |

## Structs and Enumerations

### Struct Definition

```rust
pub struct Struct {
    pub fields: BTreeMap<SmolStr, Type>,
    pub name: StructName,  // None, User, BuiltinPublic, BuiltinPrivate
}
```

### Enumeration Definition

```rust
pub struct Enumeration {
    pub name: SmolStr,
    pub values: Vec<SmolStr>,
    pub default_value: usize,  // Index in values
    pub node: Option<syntax_nodes::EnumDeclaration>,
}
```

### Accessing Enumeration Values

```slint,ignore
// In Slint code:
property<TextHorizontalAlignment> align: TextHorizontalAlignment.center;

// In compiler, lookup resolves:
// 1. "TextHorizontalAlignment" -> LookupResult::Enumeration
// 2. ".center" -> Expression::EnumerationValue { value: 1, enumeration: ... }
```

## Type Inference

### Two-Way Binding Inference

When a two-way binding is created without explicit type:

```slint,ignore
property foo <=> other.bar;  // Type inferred from other.bar
```

The type starts as `Type::InferredProperty` and is resolved during the `infer_aliases_types` pass.

### Callback Type Inference

Similarly for callback aliases:

```slint,ignore
callback my-callback <=> parent.some-callback;
```

Starts as `Type::InferredCallback` and is resolved during type inference.

## Common Patterns

### Checking Type Compatibility

```rust
if !source_type.can_convert(&target_type) {
    diag.push_error("Type mismatch", span);
}
```

### Looking Up a Property

```rust
let result = element.borrow().lookup_property("width");
if result.is_valid() {
    let ty = result.property_type;
    let visibility = result.property_visibility;
}
```

### Creating a Typed Expression

```rust
// Number with unit
Expression::NumberLiteral(100.0, Unit::Px)  // Type: LogicalLength

// Struct literal
Expression::Struct {
    ty: Type::Struct(struct_def),
    values: fields,
}
```

### Registering a Custom Type

```rust
register.insert_type(Type::Struct(Rc::new(Struct {
    fields: [("x".into(), Type::Int32)].into_iter().collect(),
    name: StructName::User { name: "MyStruct".into(), node },
})));
```

## Debugging Tips

### Type Display

All types implement `Display` for readable output:
```rust
println!("Type: {}", my_type);  // e.g., "length", "[int]", "{ x: int, y: int }"
```

### Common Type Errors

| Error | Cause | Solution |
|-------|-------|----------|
| "cannot convert X to Y" | Incompatible types | Check unit compatibility, add explicit conversion |
| "Unknown type" | Type not in register | Check import, spelling |
| "Cannot access property" | Visibility violation | Check property visibility modifier |
| "Type mismatch in binding" | Binding returns wrong type | Fix binding expression type |

### Inspecting the Type Register

```rust
// List all types
for (name, ty) in &register.types {
    println!("{}: {}", name, ty);
}

// Check if type exists
if let Some(ty) = register.lookup("MyType") {
    // ...
}
```

## Testing

```sh
# Run type system tests
cargo test -p slint-compiler langtype
cargo test -p slint-compiler lookup
cargo test -p slint-compiler typeregister

# Run all compiler tests
cargo test -p slint-compiler
```
