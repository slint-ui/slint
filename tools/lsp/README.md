 # LSP (Language Server Protocol) Server for SixtyFPS

This directory contains the implementation of the LSP server for SixtyFPS featuring diagnostics, code
completion, goto definition, and more importently, live-preview


## Usage with Visual Studio Code

For VSCode, we have an [extension in this repository](../../vscode_extension), you can install it
directly from the market place.

## Generic usage

1. Build or install the lsp binary:

```sh
cargo install sixtyfps-lsp --git https://github.com/sixtyfpsui/sixtyfps
```

2. Configure your editor to use the `$HOME/.cargo/bin/sixtyfps-lsp` binary (path may vary depending on the platform),
no arguments required

Bellow is a list of editor which have been tested:

### Kate

Install the sixtyfps-lsp binary

Kate rely on the presence of syntax highlighting file for the usage of the LSP so we'll setup
that first.
The file [sixtyfps.ksyntaxhighlighter.xml](./sixtyfps.ksyntaxhighlighter.xml) need to be copied
in a location where kate can find it:

```
mkdir -p ~/.local/share/org.kde.syntax-highlighting/syntax/
wget https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/tools/lsp/sixtyfps.ksyntaxhighlighter.xml -O ~/.local/share/org.kde.syntax-highlighting/syntax/sixtyfps.xml
```

The, to setup the LSP, install the sixtyfps-lsp binary

```
cargo install sixtyfps-lsp --git https://github.com/sixtyfpsui/sixtyfps
```

Go to *Settings > Configure Kate*. In the *Plugins* section, enable the *LSP-Client* plugin.
This will add a *LSP Client* section in the settings dialog. In that *LSP Client* section,
go to the *User Server Settings*, and  enter the following in the text area: (**Replace YOUR_USER by your username**)

```json
{
  "servers": {
	"SixtyFPS": {
	  "command": ["/home/YOUR_USER/.cargo/bin/sixtyfps-lsp"],
	  "highlightingModeRegex": "SixtyFPS"	}
  }
}
```

## QtCreator

To setup the lsp:
 1. install the sixtyfps-lsp binary with `cargo install` as explained earlier.
 2. Then in Qt creator, go to *Tools > Option* and select the *Language Client* section.
 3. Click *Add*
 4. As a name, use "SixtyFPS"
 5. use `*.60` as a file pattern. (don't use MIME types)
 6. As executable, select the `~/.cargo/bin/sixtyfps-lsp` binary (no arguments required)
 7. Click *Apply* or *Ok*

For the syntax highlighting, QtCreator also supports the same format as kate,
so refer to the instruction from the previous section to enable syntax highlighting.

In order to preview a component, when you have a .60 file open, place your cursor to
the name of the component you would like to preview and press *Alt + Enter* to open
the code action menu. Select *Show Preview* from that menu.
