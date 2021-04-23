 # LSP (Language Server Protocol) Server for SixtyFPS

This directory contains the implementation of the LSP server for SixtyFPS featuring diagnostics, code
completion, goto definition, and more importantly, live-preview


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

Bellow is a list of editors which have been tested:

### Kate

Install the sixtyfps-lsp binary

Kate relies on the presence of syntax highlighting file for the usage of the LSP so we'll setup
that first.
The file [sixtyfps.ksyntaxhighlighter.xml](./sixtyfps.ksyntaxhighlighter.xml) needs to be copied
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
 1. Install the sixtyfps-lsp binary with `cargo install` as explained in the *Generic Usage* section above.
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

<img src="https://user-images.githubusercontent.com/959326/115923547-af5eeb00-a47e-11eb-8962-5a5f011892a7.png" width="50%" height="50%">


## Vim

Vim and neovim support the Language Server Protocol via different plugins. We recommend the
[Conquer of Completion](https://github.com/neoclide/coc.nvim) plugin. Together with the
SixtyFPS LSP server, this enables inline diagnostics and code completion when editing `.60`
files.

After installing the extension, for example via [vim-plug](https://github.com/junegunn/vim-plug),
two additional configuration changes are needed to integrate the LSP server with vim:

1. Make vim recognize the `.60` files with the correct file type

In your vim configuration file (for example `~/.vimrc`) add the following to enable the
automatic recognition when opening `.60` files:

```
autocmd BufEnter *.60 :setlocal filetype=sixtyfps
```

2. Configure Conquer of Completion to use the SixtyFPS LSP server

Start `vim` and run the `:CocConfig` command to bring up the buffer that allows editing
the JSON configuration file (`coc-settings.json`), and make sure the following mapping
exists under the `language` server section:

```json
{
  "languageserver": {
    "sixtyfps": {
      "command": "sixtyfps-lsp",
      "filetypes": ["sixtyfps"]
    }
  }
}
```
