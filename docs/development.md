# SixtyFPS development guide

The build instructions are in the [building.md](./building.md) file

## Repository structures

### `helper_crates`

A set of crates that are somehow not strictly related to sixtyfps, and that could be moved to
their own repository and have their own version release at some point.

### `sixtyfps_compiler`

The main library for the compiler for .60.

Nothing in there should depends on the runtime crates.

There is a **`test`** subdirectory that contains the syntax tests.
These tests allow to test the proper error conditions.

### `sixtyfps_runtime`

The library crates that are used at runtime.

* **`corelib`** is the main library. It is meant to be used for all frontends. Ideally it should
  be kept as small as possible. **`corelib-macros`** contains some procedural macro used by corelib.
* **`rendering_backends`** contains the different backend for the different platform, separated from
  corelib.  Currently there is just the gl backend
* **`interpreter`** is the library used by the more dynamic languages backend to compile and
  interpret .60 files. It links both against corelib and the compiler lib

### `tools`

* **`compiler`** is the tool to generate the target language (e.g. c++) from the .60 files for
  frontend that have a compilation step and generated code.
* **`viewer`** is a tool that allow to open and view a .60 file.

### `api`

Here one find the frontend for different language.

### `tests`

The integration test that are testing a bunch of .60 with different frontends

### `examples`

Some manual tests

## Documentation

There are some documentations comments in the code.
HTML documentation can be generated with something like

```sh
cargo doc --document-private-items --no-deps --open
```
