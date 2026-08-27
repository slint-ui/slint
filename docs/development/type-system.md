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

The `Type` enum in `internal/compiler/langtype.rs` represents all possible types in Slint. Its
variants group into:

- **Error and placeholder types**: `Invalid` (uninitialized or error), `Void` (an expression
  returning nothing), and `InferredProperty` / `InferredCallback` for two-way bindings and
  callback aliases whose type is not resolved yet.
- **Callable types**: `Callback` and `Function`, each wrapping a `Function` signature.
- **Primitive types**: `Float32`, `Int32`, `String`, `Bool`.
- **Unit types** (dimensional quantities): `Duration` (ms, s), `PhysicalLength` (phx),
  `LogicalLength` (px, cm, mm, in, pt), `Rem`, `Angle` (deg, rad, turn, grad) and `Percent`.
- **Visual types**: `Color`, `Brush`, `Image`, `Easing`, `PathData`, `StyledText`, `MouseCursor`.
- **Composite types**: `Array`, `Struct`, `Enumeration`.
- **Special types**: `Model` (anything convertible to a model), `UnitProduct` (a list of
  unit/power pairs, e.g. px²), `ElementReference`, `ComponentFactory`, `Closure`, `Keys`,
  `DataTransfer`, and the internal `LayoutCache` and `ArrayOfU16`.

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

Elements (components/items) have their own type hierarchy.
`ElementType` (`internal/compiler/langtype.rs`) is one of: `Component` for a user-defined
component, `Builtin` for a built-in item such as `Rectangle` or `Text`, `Native` once the
`resolve_native_classes` pass has run, `Error` when the base type couldn't be looked up, and
`Global` / `Interface` for the root element of a global or an interface.

### Property Lookup on Elements

When looking up a property on an element:

1. Check the element's declared properties
2. Check inherited properties from base type
3. For built-in elements, check `BuiltinElement.properties`
4. For item types, check reserved properties (x, y, width, height, etc.)
5. Handle property aliases (deprecated names)

`ElementType::lookup_property()` does all of that and returns a `PropertyLookupResult` carrying
the resolved name, the type, the visibility and the deprecation status, plus the flags the
visibility check needs: whether the property is local to the current component and whether it
came from its direct base.

## Name Resolution (Lookup)

`LookupCtx` (`internal/compiler/lookup.rs`) carries everything needed to resolve an identifier:
the name and type of the property being bound, the type expected at the current position within
the expression, the element scope stack, the callback/function argument names, the stack of local
variable scopes, the type register and type loader, the counters that generate unique symbol
names, plus somewhere to report diagnostics and the token currently being processed.

### Lookup Order

When resolving an identifier, lookup proceeds in this order:

1. **Local variables** - Variables declared in the current scope
2. **Arguments** - Callback/function parameters
3. **Special identifiers** - `self`, `parent`, `true`, `false`
4. **Element IDs** - Named elements in the component
5. **In-scope properties** - Properties from scope stack (legacy syntax: parent properties)
6. **Global types** - Types from the type register
7. **Built-in namespaces** - `Colors`, `Easing`, `Math`, `Key`, `FontWeight`, `MouseCursor`
8. **Type-specific values** - Bare `color`, `enum` or `easing` literals (e.g. `red`, `center`), resolved against `expected_type`
9. **Built-in functions** - Unqualified global functions (`min`, `max`, `clamp`, `abs`, `debug`, ...)

The type-specific step is what makes a bare `red` mean a color: it only matches when
`expected_type` is a `color`, `brush`, `enum` or `easing`. Because it comes near the end, a
property, element or variable of the same name still takes precedence.

### Expected type

`property_type` is the type of the whole binding; `expected_type` is the type expected at the
current position within the expression. As the resolver descends into struct fields, array
elements and call arguments it updates `expected_type` (via `LookupCtx::with_expected_type`), so a
bare literal resolves against the locally expected type. This is why `property <S> s: { c: red }`
resolves `red` as a color even though the binding's type is the struct `S`.

### LookupResult

Lookup returns a `LookupResult`: an `Expression` (with an optional deprecation hint), an
`Enumeration`, a `Namespace` (a `BuiltinNamespace` value — the ones above plus the internal
`SlintInternal`), or a `Callable`. See `internal/compiler/lookup.rs`.

## Type Register

The `TypeRegister` (`internal/compiler/typeregister.rs`) maps names to property types and to
element types. Registers chain: each one can have a parent registry, and a lookup that misses
falls through to it. It also records which types are animatable, which types are only allowed
inside a given parent (so the error can say "Row can only be within a GridLayout element"), and
whether internal types should be exposed by lookups.

### Built-in Types

The register is initialized with:

1. **Primitive types**: `int`, `float`, `string`, `bool`, `color`, etc.
2. **Built-in enumerations**: `TextHorizontalAlignment`, `ImageFit`, etc.
3. **Built-in structs**: `Point`, `KeyEvent`, `PointerEvent`, etc.
4. **Built-in elements**: `Rectangle`, `Text`, `Image`, etc.

### Reserved Properties

All items automatically get reserved properties. `reserved_properties()` in
`internal/compiler/typeregister.rs` chains the per-category name/type lists — geometry (`x`, `y`,
`width`, `height`), layout (`min-width`, `preferred-height`, `horizontal-stretch`, ...), grid and
flexbox layout (`col`, `row`, `colspan`, `rowspan`, `cross-axis-self-alignment`, ...), drop and
inner shadow, transform, the deprecated rotation-origin names, and accessibility
(`accessible-role`, `accessible-label`, ...) — and yields each as a
`(name, Type, PropertyVisibility)` triple. It is a triple rather than a pair because the
visibility is not uniform: most are `Input`, but the last group covers `Output` (`absolute-position`),
`Constexpr` (`forward-focus`), `Public` and `Private` (`init`).

## Property Visibility

Properties have visibility levels that control access:

`PropertyVisibility` (`internal/compiler/object_tree.rs`) is `Private`, `Input`, `Output` or
`InOut` for ordinary properties, `Public` or `Protected` for functions, `Constexpr` for built-in
properties that must be known at compile time, and `Fake` for built-in properties that only ever
take a binding and can neither be read nor written (such as `Path`'s `commands`).

### Visibility Rules

| Visibility | Set from outside | Set from inside | Read from outside | Read from inside |
|------------|-----------------|-----------------|-------------------|------------------|
| Private    | No | Yes | No | Yes |
| Input      | Yes | No | No | Yes |
| Output     | No | Yes | Yes | Yes |
| InOut      | Yes | Yes | Yes | Yes |

## Structs and Enumerations

### Struct Definition

A `Struct` is its fields (a sorted map from name to `Type`), the resolved and constant-folded
default value of each field, and a `StructName`. The name is `None` for an anonymous struct,
`User` for one declared as `struct Foo { }` in .slint (which also keeps the declaration node, the
`@rust-attr(...)` texts and the declaration order of the fields, since the sorted field map loses
it), or `Builtin`. See `Struct` and `StructName` in `internal/compiler/langtype.rs`.

### Enumeration Definition

An `Enumeration` is its name, its values, the index of the default value within them, the
declaration node for non-builtin enums, and the `@rust-attr(...)` texts. An `EnumerationValue` is
an index into `values` plus the enumeration it belongs to.
See `internal/compiler/langtype.rs`.

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
register.insert_type(Type::Struct(Arc::new(Struct::new(
    [("x".into(), Type::Int32)].into_iter().collect(),
    struct_name,
))));
```

`Struct::new()` builds one without declared field defaults.

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
// List all types, including those from the parent registries
for (name, ty) in register.all_types() {
    println!("{name}: {ty}");
}

// Look up one type: Type::Invalid when it isn't registered
let ty = register.lookup("MyType");
```

## Testing

```sh
# Run type system tests
cargo test -p i-slint-compiler langtype
cargo test -p i-slint-compiler lookup
cargo test -p i-slint-compiler typeregister

# Run all compiler tests
cargo test -p i-slint-compiler
```
