# LSP (Language Server Protocol) Server for SixtyFPS

This directory contains the implementation of the LSP server for [SixtyFPS](https://sixtyfps.io)
featuring diagnostics, code
completion, goto definition, and more importantly, live-preview


## Usage with Visual Studio Code

For VSCode, we have an [extension in this repository](../../vscode_extension), you can install it
directly from the market place.

## Generic usage

1. Build or install the lsp binary:

```sh
cargo install sixtyfps-lsp
```
for the released version. Or, to install the develoment version:

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

Then, to setup the LSP, install the sixtyfps-lsp binary

```
cargo install sixtyfps-lsp
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

## Sublime Text

To setup the lsp:
1. Install the sixtyfps-lsp binary with `cargo install` as explained in the *Generic Usage* section above.
2. Using Package Control in Sublime Text, install the LSP package (sublimelsp/LSP)
3. Download the SixtyFPS syntax highlighting file into your User Package folder,
   e.g. on macOS `~/Library/Application Support/Sublime Text 3/Packages/User/` :
   https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/tools/lsp/sublime/SixtyFPS.sublime-syntax
4. Download the LSP package settings file into your User Package folder:
   https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/tools/lsp/sublime/LSP.sublime-settings
5. Modify the sixtyfps command path in `LSP.sublime-settings` to point to the cargo instalation path in your home folder (**Replace YOUR_USER by your username**):
   `"command": ["/home/YOUR_USER/.cargo/bin/sixtyfps-lsp"]`
6. Run "LSP: Enable Language Server Globally" or "LSP: Enable Lanuage Server in Project" from Sublime's Command Palette to allow the server to start.
7. Open a .60 file - if the server starts its name will be in the left side of the status bar.

### Enabling Live Preview
The Live Preview function can normally be activated using LSP "Code Actions",
but the Sublime Text LSP plugin has limited support for actions not associated to diagnostic reports.

In order to work around this limitation to start the Live Preview, it's also possible to do so through an LSP "Execute Command" request,
for which the root .60 file path must be passed as a parameter.
To do this, copy the provided command palette file in your User Package folder and provide an entry for each file you want to preview:
https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/tools/lsp/sublime/SixtyFPS.sublime-commands
Then select that command through the command palette.
