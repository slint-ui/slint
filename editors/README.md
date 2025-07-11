<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Editor Configuration for Slint

This folder contains extensions or configuration files for different editor to better support .slint files.
This README contains information on how to configure various editors.

If your favorite editor is not in this list, it just means we did not test it, not that it doesn't work.
We do provide a [language server for Slint](../tools/lsp) that should work with most editor that supports
the Language Server Protocol (LSP)
(see its [README.md](../tools/lsp/README.md) for more info on how to install it).
If you do test your editor with it, we would be happy to accept a pull request that adds instructions here.

## Editors

- [Visual Studio Code](#visual-studio-code)
- [Kate](#kate)
- [Qt Creator](#qtcreator)
- [Helix](#helix)
- [Vim](#vim)
- [Neovim](#neovim)
- [Sublime Text](#sublime-text)
- [JetBrains IDE](#jetbrains-ide)
- [Zed](#zed)

## Visual Studio Code

For VSCode, we have an [extension in this repository](vscode), you can install it
directly from the market place. This includes the Slint language server and is a one-stop shop to
get you started.

## Kate

Before we start, it's important to note that Kate relies on the presence of syntax highlighting file for the usage of the LSP.
Therefore, we'll set up the syntax highlighting first.

### Syntax Highlighting

The file [slint.ksyntaxhighlighter.xml](kate/slint.ksyntaxhighlighter.xml) needs to be copied into a location where Kate can find it.
See the [kate documentation](https://docs.kde.org/stable5/en/kate/katepart/highlight.html#katehighlight-xml-format)

On Linux, this can be done by running this command

```sh
mkdir -p ~/.local/share/org.kde.syntax-highlighting/syntax/
wget https://raw.githubusercontent.com/slint-ui/slint/master/editors/kate/slint.ksyntaxhighlighter.xml -O ~/.local/share/org.kde.syntax-highlighting/syntax/slint.xml
```

On Windows, download [slint.ksyntaxhighlighter.xml](./slint.ksyntaxhighlighter.xml) into `%USERPROFILE%\AppData\Local\org.kde.syntax-highlighting\syntax`

### LSP

After setting up the syntax highlighting, you can now install the Slint Language server. Check the [LSP README.md](../tools/lsp/README.md) for instructions.

Once it is installed, go to *Settings > Configure Kate*. In the *Plugins* section, enable the *LSP-Client* plugin. This will add a *LSP Client* section in the settings dialog. In that *LSP Client* section, go to the *User Server Settings*, and  enter the following in the text area:

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

To preview a component, first, position your cursor on the name definition of the component you want to preview
(for instance, `MainWindow` in `component MainWindow inherits Window {`).
Then, activate the *Show Preview* code action.
You can do this by using the Alt+Enter shortcut to bring up the code action menu,
or find it in the menu bar at *LSP Client > Code Action > Show Preview*

<img src="https://github.com/slint-ui/slint/assets/959326/e2e6f1a8-d3b8-46a1-87b3-0273c4a40cfc" width="75%" height="75%">


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
 6. As executable, select the `slint-lsp` binary (no arguments required)
 7. Click *Apply* or *Ok*

<img src="https://user-images.githubusercontent.com/959326/157453134-c1ff17ed-6c44-4a48-802f-9a9b2a57e6ab.png" width="50%" height="50%">

In order to **preview a component**, when you have a .slint file open, place your cursor to
the name of the component you would like to preview and press *Alt + Enter* to open
the code action menu. Select *Show Preview* from that menu.

## Helix

To install the Slint Language server, check the [LSP README.md](../tools/lsp/README.md).

[Helix](https://helix-editor.com/) works out of the box without further configuration. To check if Helix detects Slint Language server successfully, run this command:

```sh
hx --health slint
```

The output should be like:

```
Configured language servers:
  ✓ slint-lsp: /home/user/.local/bin/slint-lsp
Configured debug adapter: None
Configured formatter: None
Highlight queries: ✓
Textobject queries: ✓
Indent queries: ✓
```

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

The easiest way to use the language server itself in Neovim is via the `neovim/nvim-lspconfig`
and `williamboman/nvim-lsp-installer` plugins. Once these are installed
you can run `:LspInstall slint_lsp` to install the lsp binary (on Windows, Linux, and macOS).

Once the slint_lsp language server is installed and running, you can trigger the live preview
via the code actions. Unfortunately there are several ways to trigger these, so please check your
configuration.

Also, if you use `nvim-treesitter` you can install the Tree Sitter parser for Slint using `TSInstall slint`
for syntax highlighting and indentation support.

## Sublime Text

To install the Slint Language server, check the [LSP README.md](../tools/lsp/README.md).

To setup the LSP:

1. Make sure the slint language server is installed
2. Using Package Control in Sublime Text, install the LSP package (sublimelsp/LSP)
3. Download the Slint syntax highlighting files into your User Package folder,
   e.g. on macOS `~/Library/Application Support/Sublime Text/Packages/User/` :
   https://raw.githubusercontent.com/slint-ui/slint/master/editors/sublime/Slint.sublime-syntax
   https://raw.githubusercontent.com/slint-ui/slint/master/editors/sublime/Slint.tmPreferences
4. Download the LSP package settings file into your User Package folder:
   https://raw.githubusercontent.com/slint-ui/slint/master/editors/sublime/LSP.sublime-settings
5. Modify the slint-lsp command path in `LSP.sublime-settings` to point to the cargo installation path in your home folder (**Replace YOUR_USER by your username**):
   `"command": ["/home/YOUR_USER/.cargo/bin/slint-lsp"]`
6. Run "LSP: Enable Language Server Globally" or "LSP: Enable Language Server in Project" from Sublime's Command Palette to allow the server to start.
7. Open a .slint file - if the server starts its name will be in the left side of the status bar.

In order to **preview a component**, when you have a .slint file open, place your cursor to
the name of the component you would like to preview and select the "Show preview" button that
will appear on the right of the editor pane.

## JetBrains IDE

https://github.com/kizeevov/slint-idea-plugin has a plugin for the Intellij
platform.

_Note: This plugin is developed by @kizeevov._

## Zed

[Zed](https://zed.dev) is a high-performance, multiplayer code editor. The [zed-slint extension](https://github.com/slint-ui/slint/tree/master/editors/zed), originally developed and donated by Luke Jones, now lives under the slint organization. It integrates the latest release of the [slint language server](../tools/lsp/README.md) into Zed, offering code completion and syntax highlighting. Install the extension via the following steps:

1. Open the extensions tab via the Zed -> Extensions menu.
2. In the search field, enter "slint".
3. Click on "Install" for the "Slint" extension.
