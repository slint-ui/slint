# Viewer for SixtyFPS

This program is a viewer for `.slint` files from the [SixtyFPS Project](https://sixtyfps.io).

## Installation

The viewer can be installed from crates.io:

```bash
cargo install sixtyfps-viewer
```

Alternatively, you can download one of our pre-built binaries for Linux or Windows:

1. Open <https://github.com/sixtyfpsui/sixtyfps/releases>
2. Click on the latest release
3. From "Assets" download either `sixtyfps-viewer-linux.tar.gz` for a Linux x86-64 binary
   or `sixtyfps-viewer-windows.zip` for a Windows x86-64 binary.
4. Uncompress the downloaded archive and run `sixtyfps-viewer`/`sixtyfps-viewer.exe`.

## Usage

You can open .slint files by just passing it as an argument:

```bash
sixtyfps-viewer path/to/myfile.slint
```

## Command line arguments

 - `--auto-reload`: Automatically watch the file system, and reload when it changes
 - `--save-data <file>`: When exiting, write the value of public properties to a json file.
   Only property whose types can be serialized to json will be written.
   This option is incompatible with `--auto-reload`
 - `--load-data <file>`: Load the values of public properties from a json file.
 - `-I <path>`: Add an include path to look for imported .slint files or images.
 - `--style <style>`: Set the style. Defaults to `native` if the Qt backend is compiled, otherwise `fluent`
 - `--backend <backend>`: Override the SixtyFPS rendering backend
 - `--on <callback> <handler>`: Set a callback handler, see [callback handler](#callback-handlers)

Instead of a path to a file, one can use `-` for the standard input or the standard output.

## Callback handler

It is possible to tell the viewer to execute some shell commands when a callback is recieved.
You can use the `--on` command line argument, followed by the callback name, followed by the command.
Within the command arguments, `$1`, `$2`, ... will be replaced by the first, second, ... argument of the
callback. These will be shell escaped.

Example: Imagine we have a myfile.slint looking like this:

```slint
MyApp := Window {
  callback open-url(string);
  //...
}
```

It is possible to make the `open-url` callback to execute a command by doing

```bash
sixtyfps-viewer --on open-url 'xdg-open $1' myfile.slint
```

Be carefull to use single quote or to escape the `$` so that the shell don't expand the `$1`


## Dialogs

If the root element of the .slint file is a `Dialog`, the different StandardButton might close
the dialog if no callback was set on the button.

 - `ok`, `yes`, or `close` buttons accepts the dialog
 - `cancel`, `no` buttons reject the dialog

## Result code

The program returns with the following error code:
 - If the command line argument parsing fails, the exit code will be *1*
 - If the .slint compilation fails, the compilation error will be printed to stderr and the exit code
   will be *-1*
 - If a Window is closed, the exit code will be *0*
 - If a Dialog is closed with the "Ok" or "Closed" or "Yes" button, the exit code will be *0*
 - If a Dialog is closed with the "Cancel" or "No" button, or using the close button in the window
   title bar, the exit code will be *1*

## Examples

`sixtyfps-viewer` can be used to display an GUI from a shell script. For examples check out the
[examples/bash](https://github.com/sixtyfpsui/sixtyfps/tree/master/examples/bash) folder in our repository.
