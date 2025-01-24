<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

## Figma to Slint property inspector

### Features
- UI that follows Figma theme colors and light/dark mode.
- Only displays properties for a single selected item. Warns if nothing or multiple items are selected.
- Displayed properties are re-written to fit Slint syntax.
- Copy to clipboard button.
- Don't show copy if there are no properties.

### Future tweaks
- Set plugin up to build via Vite. Figma only works with a single *.js file. Using Vite means we
can have multiple files, import libraries, use react, etc and it will sort out a bundle that just becomes one *.js file.
- Can Shiki be used to syntax color the properties?
- Still some missing properties and thought needed for how to deal with Figma properties that don't exist in Slint e.g. you can have unlimited individual shadows and gradients. Slint only supports one of each.
- Grab the 'text' value for any text from the original data.


Below are the steps to get your plugin running. You can also find instructions at:

  https://www.figma.com/plugin-docs/plugin-quickstart-guide/

Enusure you have the desktop version of Figma installed.

First install the dependencies with `pnpm i`.
Then build the project with `pnpm build`.
Then in Figma select Plugins > Plugins & Widgets > Import from manifest... and then chose the manifest.json from this folder.

