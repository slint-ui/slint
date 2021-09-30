# Viewer for SixtyFPS

This program is a viewer for `.60` files from the [SixtyFPS Project](https://sixtyfps.io).

## Usage

The viewer can be installed from crates.io:

```bash
cargo install sixtyfps-viewer
```

Then you can open .60 files by just passing it as an argument:

```bash
sixtyfps-viewer path/to/myfile.60
```

## Command line arguments

 - `--auto-reload`: Automatically watch the file system, and reload when it changes
 - `--save-data <file>`: When exiting, write the value of public properties to a json file.
   Only property whose types can be serialized to json will be written.
   This option is incompatible with `--auto-reload`
 - `--load-data <file>`: Load the values of public properties from a json file.
 - `-I <path>`: Add an include path to look for imported .60 files or images.
 - `--style <style>`: Set the style. Defaults to `native` if the Qt backend is compiled, otherwise `fluent`
 - `--backend <backend>`: Override the SixtyFPS rendering backend

Instead of a path to a file, one can use `-` for the standard input or the standard output.

## Dialogs

If the root element of the .60 file is a `Dialog`, the different StandardButton might close 
the dialog if no callback was set on the button.

 - `ok`, `yes`, or `close` buttons accepts the dialog
 - `cancel`, `no` buttons reject the dialog

## Result code

The program returns with the following error code:
 - If the command line argument parsing fails, the exit code will be *1*
 - If the .60 compilation fails, the compilation error will be printed to stderr and the exit code
   will be *-1*
 - If a Window is closed, the exit code will be *0*
 - If a Dialog is closed with the "Ok" or "Closed" or "Yes" button, the exit code will be *0*
 - If a Dialog is closed with the "Cancel" or "No" button, or using the close button in the window
   title bar, the exit code will be *1*
