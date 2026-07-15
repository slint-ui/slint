# Slint tests

This document describes the testing infrastructure of Slint.

## Workspace layout

Most of the test crates below (`test-driver-*`, `doctests`, `test-driver-screenshots`)
live in the separate `tests/` Cargo workspace, not the root workspace. Running their
`cargo test -p <crate>` commands from the repository root requires
`--manifest-path tests/Cargo.toml`, as shown below. `tests/run_tests.sh` already passes
this for you, so prefer it when it covers your driver (see the Rust driver section).

## Syntax tests

The syntax tests check that the compiler shows the right error messages in case of error.

The syntax tests are located in [internal/compiler/tests/syntax/](../internal/compiler/tests/syntax/) and it's driven by the
[`syntax_tests.rs`](../internal/compiler/tests/syntax_tests.rs) file. More info in the comments of that file.

In summary, each .slint file has comments with `> <error` like so:

```ignore
foo bar
//  > <error{parse error}
```

Meaning that there must be an error on the line above spanning `bar`, as indicated by the `>` and `<` arrows.

Ideally, each error message must be tested like so.

The syntax test can be run alone with

```sh
cargo test -p i-slint-compiler --features display-diagnostics --test syntax_tests
```

In order to update the failing tests, set the `SLINT_SYNTAX_TEST_UPDATE` environment variable to `1`.
```sh
SLINT_SYNTAX_TEST_UPDATE=1 cargo test -p i-slint-compiler --test syntax_tests
```
This will change the comments to add the error of the expected messages


## Driver tests

These tests make sure that features in .slint behave as expected.
All the .slint files in the sub directories will be tested by the drivers with the different
language frontends.

The `.slint` code contains a comment with some block of code which is extracted by the relevant driver.

`tests/run_tests.sh <rust|cpp|interpreter|python|nodejs> [<filter>]` is the convenient entry
point for all five drivers below and passes `--manifest-path tests/Cargo.toml` for you; the
`cargo test -p test-driver-*` commands shown per driver are the equivalent direct invocations.

### Interpreter test

The interpreter test is the faster test to compile and run. It tests the compiler and the eval feature
as run by the viewer or such. It can be run like so:

```
cargo test --manifest-path tests/Cargo.toml -p test-driver-interpreter --
```

You can add an argument to test only for particular tests.

If the last component in the file includes a public `bool` property named `test` (declared
`out` or `in-out`), the test will verify that its value is `true`. A `private` (the default)
or `in` property is invisible to the driver, so the test passes vacuously without actually
checking anything.

example:

```slint
export component Foo inherits Rectangle {
   // test would fail if that property was false
   in-out property <bool> test: 1 + 1 == 2;
}
```

### Rust driver

The rust driver will compile each snippet of code and put it in a `slint!` macro in its own module
In addition, if there are ```` ```rust ```` blocks in a comment, they are extracted into a `#[test]`
function in the same module. This is useful to test the rust api.
The `SLINT_TEST_FILTER` environment variable can be set while building to only build the tests
that matches the filter.
Example: to run all the layout tests:

```
SLINT_TEST_FILTER=layout cargo test --manifest-path tests/Cargo.toml -p test-driver-rust
```
or
```
tests/run_tests.sh rust layout
```

Instead of putting everything in a slint! macro, it's possible to tell the driver to do the
compilation in the build.rs, with the build-time feature:

```
SLINT_TEST_FILTER=layout cargo test --manifest-path tests/Cargo.toml -p test-driver-rust --features build-time
```
or
```
tests/run_tests.sh rust layout --features build-time
```

### C++ driver

The C++ test driver will take each .slint and generate a .h for it. It will also generate a .cpp that
includes it, and add the ```` ```cpp ```` block in the main function.
Each program is compiled separately. And then run.

Some macro like `assert_eq` are defined to look similar to the rust equivalent.

It requires the Slint C++ library to be built first (see
[building.md](./building.md#c-tests)):

```
cargo build --lib -p slint-cpp
cargo test --manifest-path tests/Cargo.toml -p test-driver-cpp --
```

Note that there are also C++ unit tests that can be run by CMake

### Node driver

This is used to test the NodeJS API. It takes the ```` ```js ```` blocks in comment and make .js file
with it that loads the .slint and runs node with it.
Each test is run in a different node process.

```
cargo test --manifest-path tests/Cargo.toml -p test-driver-nodejs
```

### Python driver

This is used to test the Python API. It compiles each `.slint` file with `OutputFormat::Python`,
then runs the generated `.py` file as a subprocess via `uv run`, which loads the `slint` Python
module and re-compiles the source using `slint-interpreter`.

```
cargo test -p test-driver-python
```

See [docs/development/python-tests.md](development/python-tests.md) for the full picture,
including how to rebuild `slint-python` after making changes.

## Screenshot tests

This is used to test renderer backends. It supports the `SoftwareRenderer` (with and without
embedded assets) and the Skia renderer, selected via the `software`, `software-embed-assets`,
and `skia` Cargo features (all enabled by default). Each `.slint` file in
`tests/screenshots/cases` will be loaded, rendered with each enabled renderer, and the results
will be compared to the reference images in the matching `tests/screenshots/references/<renderer>`
sub-directory.

To generate references images for all test files in `tests/screenshots/cases` run:

```
SLINT_CREATE_SCREENSHOTS=1 cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots
```

To start the tests run and compare images:

```
cargo test --manifest-path tests/Cargo.toml -p test-driver-screenshots
```

## Embedded MCP Server

The testing backend includes an embedded MCP (Model Context Protocol) server that allows
AI coding tools (e.g. Claude Code) to inspect and interact with a running Slint application
in real time. Enable the `mcp` Cargo feature on the `slint` crate and set
`SLINT_MCP_PORT` to start the server.

See the [testing backend README](../internal/backends/testing/README.md) for usage instructions
and [docs/development/mcp-server.md](development/mcp-server.md) for architecture details.

## Doctests

```
cargo test --manifest-path tests/Cargo.toml -p doctests
```

The doctests extract the ```` ```slint ```` snippets from the files in the docs folder and make sure
they can be built without errors.
