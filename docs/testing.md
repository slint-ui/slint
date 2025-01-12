<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Slint tests

This documents describe the testing infrastructure of Slint

## Syntax tests

The syntax tests are testing that the compiler show the right error messages in case of error.

The syntax tests are located in [internal/compiler/tests/syntax/](../internal/compiler/tests/syntax/) and it's driven by the
[`syntax_tests.rs`](../internal/compiler/tests/syntax_tests.rs) file. More info in the comments of that file.

In summary, each .slint files have comments with `^error` like so:

```ignore
foo bar
//  ^error{parse error}
```

Meaning that there must be an error on the line above at the location pointed by the caret.

Ideally, each error message must be tested like so.

The syntax test can be run alone with

```sh
cargo test --test syntax_tests
```

## Driver tests

These tests make sure that feature in .slint behave as expected.
All the .slint files in the sub directories are going to be test by the drivers with the different
language frontends.

The `.slint` code contains a comment with some block of code which is extracted by the relevant driver.

### Interpreter test

The interpreter test is the faster test to compile and run. It test the compiler and the eval feature
as run by the viewer or such. It can be run like so:

```
cargo test -p test-driver-interpreter --
```

You can add an argument to test only for particular tests.

If the last component in the file includes a `bool` property named `test`, the test will verify that its value is `true`.

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
This is all compiled in a while program, so the `SLINT_TEST_FILTER` environment variable can be
set while building to only build the test that matches the filter.
Example: to test all the layout test:

```
SLINT_TEST_FILTER=layout cargo test -p test-driver-rust
```

Instead of putting everything in a slint! macro, it's possible to tell the driver to do the
compilation in the build.rs, with the builod-time feature:

```
SLINT_TEST_FILTER=layout cargo test -p test-driver-rust --features build-time
```

### C++ driver

The C++ test driver will take each .slint and generate a .h for it. It will also generate a .cpp that
includes it, and add the ```` ```cpp ```` block in the main function.
Each program is compiled separately. And then run.

Some macro like `assert_eq` are defined to look similar to the rust equivalent.

```
cargo test -p  test-driver-cpp --
```

Note that there are also C++ unit tests that can be run by CMake

### Node driver

This is used to test the NodeJS API. It takes the ```` ```js ```` blocks in comment and make .js file
with it that loads the .slint and runs node with it.
Each test is run in a different node process.

```
cargo  test -p test-driver-nodejs
```

## Screenshot tests

This is used to test renderer backends. At the moment it supports the `SoftwareRenderer`. Each `.slint` file in `tests/screenshots/cases` will be loaded
rendered and the results will be compared to the reference images in `tests/screenshots/references`.

To generate references images for all test files in `tests/screenshots/cases` run:

```
SLINT_CREATE_SCREENSHOTS=1 cargo test -p test-driver-screenshots
```

To start the tests run and compare images:

```
cargo test -p test-driver-screenshots
```

## Doctests

```
cargo test -p doctests
```

The doctests extracts the ```` ```slint ````  from the files in the docs folder and make  sure that
the snippets can be build without errors
