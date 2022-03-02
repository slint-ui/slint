# Editor Configuration for Slint

## Visual Studio Code

For VSCode, we have an [extension in this repository](vscode_extension), you can install it
directly from the market place. This includes the Slint language server and is a one-stop shop to
get you started.

## Kate

### Syntax Highlighting

Kate relies on the presence of syntax highlighting file for the usage of the LSP so we'll setup
that first. The file [slint.ksyntaxhighlighter.xml](kate/slint.ksyntaxhighlighter.xml) needs to be copied
in a location where kate can find it. See the [kate documentation](https://docs.kde.org/stable5/en/kate/katepart/highlight.html#katehighlight-xml-format)

On Linux, this can be done by running this command

```
mkdir -p ~/.local/share/org.kde.syntax-highlighting/syntax/
wget https://raw.githubusercontent.com/slint-ui/slint/master/editor/kate/slint.ksyntaxhighlighter.xml -O ~/.local/share/org.kde.syntax-highlighting/syntax/slint.xml
```

On Windows, download [slint.ksyntaxhighlighter.xml](./slint.ksyntaxhighlighter.xml) into `%USERPROFILE%\AppData\Local\org.kde.syntax-highlighting\syntax`


### LSP

To install the Slint Language server, check the [LSP README.md](../tools/lsp/README.md).

Then go to *Settings > Configure Kate*. In the *Plugins* section, enable the *LSP-Client* plugin.
This will add a *LSP Client* section in the settings dialog. In that *LSP Client* section,
go to the *User Server Settings*, and  enter the following in the text area:

```json
{
  "servers": {
    "Slint": {
      "path": ["%{ENV:HOME}/.cargo/bin", "%{ENV:USERPROFILE}/.cargo/bin"],
      "command": ["slint-lsp"],
      "highlightingModeRegex": "Slint"
    }
  }
}
```

To show the preview, right click on the name definition of the component you want to preview
(eg. `MainWindow` in `MainWindow := Window {`). Then in the menu, select *LSP Client > Code Action > Show Preview*.


## QtCreator

### Syntax Highlighting

For the **syntax highlighting**, QtCreator supports the same format as Kate, with
the [xml file](kate/slint.ksyntaxhighlighter.xml) at the same location.
Refer to the instruction from the [previous section](#syntax-highlighting) to enable syntax highlighting.

### LSP

To install the Slint Language server, check the [LSP README.md](../tools/lsp/README.md).

To setup the lsp:

 1. Install the `slint-lsp` binary
 2. Then in Qt creator, go to *Tools > Option* and select the *Language Client* section.
 3. Click *Add*
 4. As a name, use "Slint"
 5. use `*.slint` as a file pattern. (don't use MIME types)
 6. As executable, select the `~/.cargo/bin/slint-lsp` binary (no arguments required)
 7. Click *Apply* or *Ok*

<img src="https://user-images.githubusercontent.com/959326/115923547-af5eeb00-a47e-11eb-8962-5a5f011892a7.png" width="50%" height="50%">

In order to **preview a component**, when you have a .slint file open, place your cursor to
the name of the component you would like to preview and press *Alt + Enter* to open
the code action menu. Select *Show Preview* from that menu.

## Vim

To install the Slint Language server, check the [LSP README.md](../tools/lsp/README.md).

Vim support the Language Server Protocol via its [Conquer of Completion](https://github.com/neoclide/coc.nvim)
plugin. Together with the Slint LSP server, this enables inline diagnostics and code completion when
editing `.slint` files.

After installing the extension, for example via [vim-plug](https://github.com/junegunn/vim-plug),
two additional configuration changes are needed to integrate the LSP server with vim:

1. Make vim recognize the `.slint` files with the correct file type

Install the `slint-ui/vim-slint` plugin.

Alternatively you can add the following to your vim configuration file (e.g. `vimrc`) to
enable automatic recognition of `.slint` files:

```
autocmd BufEnter *.slint :setlocal filetype=slint
```

2. Make sure the slint language server is installed and can be found in PATH.

3. Configure Conquer of Completion to use the Slint LSP server

Start `vim` and run the `:CocConfig` command to bring up the buffer that allows editing
the JSON configuration file (`coc-settings.json`), and make sure the following mapping
exists under the `language` server section:

```json
{
  "languageserver": {
    "slint": {
      "command": "slint-lsp",
      "filetypes": ["slint"]
    }
  }
}
```

### Neovim

Follow step 1. of the Vim section to get support for `.slint` files.

The easist way to use the language server itself is in Neovim is via the `neovim/lsp-config`
and `williamboman/nvim-lsp-installer` plugins. Once these are installed
you can run `:LspInstallInfo` to install the `slint_lsp` binary (on Windows and Linux).

Once the slint_lsp language server is installed and running, you can triggger the live preview
via the code actions. Unfortunately there are several ways to trigger these, so please check your
configuration.

You might also find the `jrmoulton/tree-sitter-slint` tree-sitter parser for the Slint language
interesting. Unfortunately this is not integrated into the tree-sitter support of Neovim at this
time.

## Sublime Text

To install the Slint Language server, check the [LSP README.md](../tools/lsp/README.md).

To setup the LSP:
1. Make sure the slint language server is installed
2. Using Package Control in Sublime Text, install the LSP package (sublimelsp/LSP)
3. Download the Slint syntax highlighting files into your User Package folder,
   e.g. on macOS `~/Library/Application Support/Sublime Text/Packages/User/` :
   https://raw.githubusercontent.com/slint-ui/slint/master/editor/sublime/Slint.sublime-syntax
   https://raw.githubusercontent.com/slint-ui/slint/master/editor/sublime/Slint.tmPreferences
4. Download the LSP package settings file into your User Package folder:
   https://raw.githubusercontent.com/slint-ui/slint/master/editor/sublime/LSP.sublime-settings
5. Modify the slint-lsp command path in `LSP.sublime-settings` to point to the cargo instalation path in your home folder (**Replace YOUR_USER by your username**):
   `"command": ["/home/YOUR_USER/.cargo/bin/slint-lsp"]`
6. Run "LSP: Enable Language Server Globally" or "LSP: Enable Lanuage Server in Project" from Sublime's Command Palette to allow the server to start.
7. Open a .slint file - if the server starts its name will be in the left side of the status bar.

In order to **preview a component**, when you have a .slint file open, place your cursor to
the name of the component you would like to preview and select the "Show preview" button that
will appear on the right of the editor pane.
