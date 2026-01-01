# Slint Updater

A syntax-aware migration tool that automatically upgrades `.slint` files to the latest Slint language syntax.

As the Slint language evolves, some syntax patterns are deprecated in favor of new ones. This tool parses your `.slint` files using the Slint compiler and applies transformations to migrate deprecated patterns.

## Installation

```bash
cargo install slint-updater
```

## Usage

```bash
# Preview changes (prints to stdout)
slint-updater file.slint

# Apply changes in-place
slint-updater -i file.slint

# Update multiple files
slint-updater -i src/**/*.slint

# Move property declarations to component root
slint-updater -i --move-declarations file.slint
```

### Options

| Flag | Description |
|------|-------------|
| `-i, --inline` | Modify files in-place instead of printing to stdout |
| `--move-declarations` | Move all property declarations to the root of each component |

## Supported File Types

- `.slint` files
- `.rs` files containing `slint!` macros
- `.md` files with ` ```slint ` code blocks

## Transformations

### Active Transforms

**Enum renames:**
- `PointerEventButton.none` → `PointerEventButton.other`
- `Keys.*` → `Key.*`

### Experimental Transforms

These transforms handle migrations from older Slint syntax versions:

**Component declaration syntax:**
```slint,no-test
// Old syntax
MyComponent := Rectangle { }

// New syntax
component MyComponent inherits Rectangle { }
```

**Property visibility:**
```slint,no-test
// Old syntax
property <int> count: 0;

// New syntax
in-out property <int> count: 0;
```

**Struct declaration:**
```slint,no-test
// Old syntax
MyStruct := { field: int }

// New syntax
struct MyStruct { field: int }
```

## Examples

Update all `.slint` files in a project:
```bash
slint-updater -i $(find . -name "*.slint")
```

Preview changes before applying:
```bash
slint-updater file.slint | diff file.slint -
```

Update Slint code embedded in Rust:
```bash
slint-updater -i src/**/*.rs
```
