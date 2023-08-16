<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial -->
# figma_import: Figma to Slint import tool

This tool imports a design from Figma into a .slint file.

## Get a Token from figma

When logged into Figma, go to "Account settings"
(from the "Help and account" submenu of the hamburger menu, Or click on your name, then "Settings")

In the section "Personal access tokens", click on "Create a new personal access token",
enter a description of your choice, and then copy the token in the yellow frame.

## Exporting a file

The file ID is the string of character that is in the URL when you have an open design, for example, the url look like this:
`https://www.figma.com/file/XxxxxxXXXxxXX/Some-description/...`

and the File ID is that `XxxxxxXXXxxXX` string.

Then you can export that file to a .slint by running the command

```sh
cargo run -- --token aaaaaa-bbbbbbbb-cccc-dddd-eeee-ffffffffffff XxxxxxXXXxxXX
```

With the right token and file id.

This will create a `figma_output` directory with a `main.slint` file and some images.

Other options:
* `--node <id>` to generate a specific node (eg: "123:12")
* `--child <index>` to generate from one of the direct children of the canvas.
