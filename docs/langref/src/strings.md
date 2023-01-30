## Strings

Strings can be used with surrounding quotes: `"foo"`.

Some character can be escaped with slashes (`\`)

| Escape          | Result                                                                                                |
| --------------- | ----------------------------------------------------------------------------------------------------- |
| `\"`            | `"`                                                                                                   |
| `\\`            | `\`                                                                                                   |
| `\n`            | new line                                                                                              |
| `\u{xxx}`       | where `xxx` is an hexadecimal number, this expand to the unicode character represented by this number |
| `\{expression}` | the expression is evaluated and inserted here                                                         |

Anything else after a `\` is an error.

(TODO: translations: `tr!"Hello"`)

```slint,no-preview
export component Example inherits Text {
    text: "hello";
}
```
