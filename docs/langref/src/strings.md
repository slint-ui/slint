## Strings

Any sequence of utf-8 encoded characters surrounded by quote is a `string`: `"foo"`.

Escape sequences may be embedded into strings to insert characters that would
be hard to insert otherwise:

| Escape          | Result                                                                                          |
| --------------- | ----------------------------------------------------------------------------------------------- |
| `\"`            | `"`                                                                                             |
| `\\`            | `\`                                                                                             |
| `\n`            | new line                                                                                        |
| `\u{x}`         | where `x` is a hexadecimal number, expands to the unicode code point represented by this number |
| `\{expression}` | the result of evaluating the expression                                                         |

Any other unicode code point following an unescaped `\` is an error.

```slint,no-preview
export component Example inherits Text {
    text: "hello"
}
```
