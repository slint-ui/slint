<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Contributing

Contributions to the tree-sitter parser are wanted to fix currently failing tests and to test any other regressions.
New tests are also needed for syntax highlighting

The `tree-sitter` cli tool is needed to generate the parser and run the tests.

To get started run `tree-sitter generate` to generate the parser.

## Testing

Tests for the tree-sitter code are not currently automatically tested with CI but testing is done with the `tree-sitter test` cli command.

## Coding Style

The style of the test can be created automatically using the tree-sitter cli.
Once all test are passing running `tree-sitter test -u` will update all tests to the current test output and format and style them.
This should be used carefully though because this command will make even failing tests pass

## Test Status

While there are failing tests this list should be kept up to date to ensure there aren't any new regressions
Once all tests are passing this list can be removed.

- [ ] comments:
  - [x] A single line comment
  - [x] A multi-line comment
  - [ ] A nested comment
- [ ] callbacks:
  - [x] Setting a callback
  - [x] Declare a callback
  - [x] Declare callback with parameters
  - [ ] Set a callback with parameters
- [ ] structs:
  - [ ] Anonymous struct
  - [x] Named struct
  - [ ] List of structs
- [ ] statements:
  - [x] Import statement
  - [x] Global Singleton
  - [x] Export statement
  - [x] More complicated conditional statement
  - [ ] For-in statement
  - [ ] For-in statement with anonymous struct as property
  - [x] Animation statement
  - [ ] Animate two variables together statement
  - [x] State statement
  - [x] Transition statement
- [ ] components:
  - [x] A basic window
  - [x] Visibility modifier
  - [x] A window with a sub component
  - [x] Setting a property
  - [x] Property declaration
  - [ ] Two way binding
  - [x] Relative values
  - [x] Define and set a property
  - [x] Named sub component
  - [x] Conditional named component
  - [x] Conditional unnamed component
- [ ] expressions:
  - [x] Relative properties
  - [x] Ternary Expression
  - [x] Chained ternary Expression
  - [ ] Arrays as expressions
  - [x] String expression
  - [ ] Color expression
  - [ ] Brush expression
  - [ ] Function expression
  - [x] Image expression
  - [x] Empty expression
  - [ ] Empty expression with semi-colon

## Contributor License Agreement

The same license agreement exists for the tree-sitter parser as the rest of the repository

See: [Contributor License Agreement (CLA)](https://cla-assistant.io/slint-ui/slint)
