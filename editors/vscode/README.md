<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

# Slint for Visual Studio Code

This extension for VS Code adds support for the [Slint](https://slint.dev) design markup language.

## Features

-   Syntax highlighting
-   Diagnostics from .slint files
-   Live Preview of a .slint file
-   Completion of properties
-   Jump to definition (currently, only definition of Component)

## Installation

You can install the extension directly from the [Visual Studio Code Marketplace](https://marketplace.visualstudio.com/items?itemName=Slint.slint). Afterwards it is
automatically activated when editing files with the `.slint` extension.

## Live-Preview

In addition to the usual code editing features such as completion and syntax highlighting, this extension
also offers the ability to view a rendering of the file you're editing and update it on-the-fly when making
changes.

You can issue the "Slint: Show Preview" command from the command palette when editing a `.slint` file. This
will create a new top-level window that renders the file you're editing. Any changes you make are immediately
visible, it is not necessary to save the file.

## Reporting Issues

Issues should be reported in the [Slint issue tracker](https://github.com/slint-ui/slint/labels/vscode-extension).

