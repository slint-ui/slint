# Compiler & Runtime Internals

> Note for AI coding assistants (agents):
> **When to load this document:** Working on compiler passes, code generation,
> property bindings, the reactive system, or adding new language features.
> For general build commands and project structure, see `/AGENTS.md`.

## Compiler Pipeline

The Slint compiler lowers `.slint` source files to a Low-Level Representation (LLR).
Both code generators and the interpreter consume this IR:

```mermaid
flowchart LR
    A[".slint Source"] --> B["Lexer"]
    B --> C["Parser"]
    C --> D["Object Tree"]
    D --> E["Passes"]
    E --> F["LLR"]
    F --> G["Code Generators"]
    G --> H["Rust / C++ / etc."]
    F --> I["Interpreter"]
```

| Stage | Location | Description |
|-------|----------|-------------|
| **Lexer** | `internal/compiler/lexer.rs` | Tokenizes `.slint` source into tokens |
| **Parser** | `internal/compiler/parser.rs` | Builds syntax tree from tokens |
| **Object Tree** | `internal/compiler/object_tree.rs` | High-level IR representing components and elements |
| **Passes** | `internal/compiler/passes.rs`, `internal/compiler/passes/` | Validation, lowering, and optimization of the object tree |
| **LLR** | `internal/compiler/llr/` | Shared IR for code generation and interpretation, with its own optimization passes |
| **Generators** | `internal/compiler/generator/` | Target-specific code generators (Rust, C++, etc.) |
| **Interpreter** | `internal/interpreter/` | Instantiates components from LLR and evaluates LLR expressions |

## Compiler Passes

`internal/compiler/passes.rs` defines the object-tree pass order.
Some passes run conditionally or more than once; use `run_import_passes` and `run_passes` as the source of truth.
The following groups highlight common entry points rather than listing every pass.

### 1. Import Passes (`run_import_passes`)

- `infer_aliases_types` - Resolve type aliases
- `validate_interfaces` - Validate interface implementations
- `resolving` - Resolve expressions, types, and references
- `purity_check` - Verify function purity
- `check_expressions` - Validate expression semantics
- `redundant_property_bindings` - Report bindings that set a property to its default value.
  It runs here so that the LSP reports it, and sees the bindings as written.

### 2. Transformation Passes (main `run_passes`)

- `inject_debug_hooks` - Add debugging support before lowering and inlining
- `lower_*` passes - Transform high-level constructs (states, layouts, popups, etc.)
- `inlining` - Inline components as needed
- `collect_*` passes - Gather globals, structs, subcomponents
- `focus_handling` - Set up focus navigation
- `default_geometry` - Calculate default sizes

### 3. Object-Tree Optimization (within `run_passes`)

- `const_propagation` - Propagate constant values
- `remove_aliases` - Eliminate property aliases
- `remove_unused_properties` - Dead code elimination
- `deduplicate_property_read` - Optimize property access
- `optimize_useless_rectangles` - Remove unnecessary elements

After these passes, `llr::lower_to_item_tree::lower_to_item_tree` produces a `CompilationUnit` and runs the LLR optimizations.
`internal/compiler/llr.rs` defines their order: count property uses, inline simple expressions, then remove unused properties, callbacks, and functions.

## Property Binding & Reactivity

Slint's reactive property system is implemented in `internal/core/properties.rs`:

```mermaid
flowchart TD
    A["Property A"] -->|"dependency"| B["Binding"]
    B -->|"evaluates"| C["Property B"]
    C -->|"notifies"| D["Dependents"]
    D -->|"re-evaluate"| B
```

**Key concepts:**
- **Properties** (`Property<T>`) hold values and track dependencies
- **Bindings** are expressions that compute property values
- **Dependency tracking** uses a doubly-linked list (`DependencyListHead`/`DependencyNode`)
- When a property changes, all dependent bindings are marked dirty and re-evaluated

The binding evaluation is lazy - properties are only recomputed when read after being marked dirty.

## Interpreter vs Compiled Modes

Slint supports two execution modes that share the compiler passes and LLR lowering:

| Mode | Entry Point | Use Case |
|------|-------------|----------|
| **Compiled** | `slint!` macro, `slint-build` | Production apps, maximum performance |
| **Interpreted** | `slint-interpreter` crate | Runtime `.slint` loading, tooling, scripting |

The compiled path generates static Rust/C++ code from LLR at build time.
The interpreter compiles `.slint` at runtime and lowers the resulting document through the same `lower_to_item_tree` entry point.
`build_from_document` in `internal/interpreter/component.rs` stores the resulting `llr::CompilationUnit` in component definitions.
`eval_expression` in `internal/interpreter/eval.rs` walks `llr::Expression` values at runtime.

## Key Data Structures

| Structure | Location | Purpose |
|-----------|----------|---------|
| `Document` | `compiler/object_tree.rs` | Root of parsed `.slint` file |
| `Component` | `compiler/object_tree.rs` | A component definition |
| `Element` | `compiler/object_tree.rs` | An element within a component |
| `Expression` | `compiler/expression_tree.rs` | Expressions used by object-tree passes |
| `llr::Expression` | `compiler/llr/expression.rs` | Lowered expressions consumed by generators and the interpreter |
| `Type` | `compiler/langtype.rs` | Type system representation |
| `CompilationUnit` | `compiler/llr/item_tree.rs` | LLR output shared by code generators and the interpreter |

## Common Modification Patterns

### Adding a New Built-in Element

1. **Declare the element** in `internal/compiler/builtin_elements.rs`
2. **Add runtime item** in `internal/core/items/` (new file or existing)
3. **Register the item** in `internal/core/items.rs` (add to `ItemVTable`)
4. **Update type registry** in `internal/compiler/typeregister.rs`
5. **Add rendering support** in each renderer (`internal/renderers/*/`)
6. **Add tests** in `tests/cases/elements/`

### Adding a New Compiler Pass

1. **Create pass file** in `internal/compiler/passes/your_pass.rs`
2. **Declare the module and register it in the pipeline**, both in
   `internal/compiler/passes.rs` (choose appropriate phase)
3. **Add tests** - either unit tests in the pass file or `.slint` test cases

### Adding a New Property Type

1. **Define type** in `internal/compiler/langtype.rs`
2. **Add parsing support** if new syntax needed
3. **Handle in relevant passes** (type checking, lowering)
4. **Add runtime support** in `internal/core/` if needed
5. **Update code generators** in `internal/compiler/generator/`

## Debugging Tips

### Inspecting Generated Code

To see the Rust code that the compiler generates from a `.slint` file:

```sh
cargo run -p slint-compiler -- -f rust path/to/file.slint | rustfmt > path/to/file.slint.rs
```

To see the generated C++ code:
```sh
cargo run -p slint-compiler -- -f cpp path/to/file.slint > path/to/file.slint.cpp
```

This is invaluable when debugging code generation issues — you can see exactly what the generators emit without running a full build of an application.

### Binding Loop Chains

A binding loop diagnostic shows each property the compiler invents, such as `layoutinfo-h` and `layout-cache-v`, as only its element,
because the source can never name them.
Set `SLINT_FULL_BINDING_LOOP_CHAIN` to keep them, which is what you want when the loop is the compiler's own doing:

```sh
SLINT_FULL_BINDING_LOOP_CHAIN=1 cargo run -p slint-compiler -- -f rust -o /dev/null path/to/file.slint
```
