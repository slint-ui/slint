# SixtyFPS-fmt

This tool for formatting .60 syntax is in a very early stage.
There might be certain parts of the language that are not yet supported.
If you find any such examples, please open an issue including the example and the expected output.

## Building

Use `cargo build --release` or similar to build this crate.

## Usage

The built binary can be used in following ways:

- `sixtyfps-fmt <path>` - reads the file and outputs the formatted version to stdout
- `sixtyfps-fmt -i <path>` - reads the file and saves the output to the same file
- `sixtyfps-fmt /dev/stdin` - using /dev/stdin you can achieve the special behavior
  of reading from stdin and writing to stdout

Note that `.60` files are formatted, while `.md` and `.rs` files are searched for `.60` blocks.
All other files are left untouched.

## Usage with VSCode

While we don't yet have a proper VSCode integration for this formatter,
here is a simple way how you can get around it.

1. Install the extension Custom Format by Vehmloewff. [Marketplace link](https://marketplace.visualstudio.com/items?itemName=Vehmloewff.custom-format)
2. Build sixtyfps-fmt locally.
3. Add a section like this to your vscode `settings.json`:
```
{
  "custom-format.formatters": [
    {
      "language": "sixtyfps",
      "command": "/path/to/your/built/sixtyfps-fmt /dev/stdin"
    }
  ]
}
```
4. (Optional) Allow formatting or save, or set this formatter as default for .60 files.
5. Enjoy! Your .60 files are now formatted.
