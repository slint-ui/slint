<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial -->

# Slint-fmt

This tool for formatting .slint syntax is in a very early stage.
There might be certain parts of the language that are not yet supported.
If you find any such examples, please open an issue including the example and the expected output.

## Building

This tool is part of slint-lsp. See the README file in `lsp` for building information.

## Usage

The built binary can be used in following ways:

- `slint-lsp format <path>` - reads the file and outputs the formatted version to stdout
- `slint-lsp format -i <path>` - reads the file and saves the output to the same file
- `slint-lsp format /dev/stdin` - using /dev/stdin you can achieve the special behavior
  of reading from stdin and writing to stdout

Note that `.slint` files are formatted, while `.md` and `.rs` files are searched for `.slint` blocks.
All other files are left untouched.

## Usage with VSCode

While we don't yet have a proper VSCode integration for this formatter,
here is a simple way how you can get around it.

1. Install the extension Custom Format by Vehmloewff. [Marketplace link](https://marketplace.visualstudio.com/items?itemName=Vehmloewff.custom-format)
2. Build slint-lsp locally.
3. Add a section like this to your vscode `settings.json`:
```
{
  "custom-format.formatters": [
    {
      "language": "slint",
      "command": "/path/to/your/built/slint-lsp format /dev/stdin"
    }
  ]
}
```
4. (Optional) Allow formatting or save, or set this formatter as default for .slint files.
5. Enjoy! Your .slint files are now formatted.
