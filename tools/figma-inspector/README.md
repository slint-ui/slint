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

This plugin template uses Typescript and NPM, two standard tools in creating JavaScript applications.

First, download Node.js which comes with NPM. This will allow you to install TypeScript and other
libraries. You can find the download link here:

  https://nodejs.org/en/download/

Next, install TypeScript using the command:

  npm install -g typescript

Finally, in the directory of your plugin, get the latest type definitions for the plugin API by running:

  npm install --save-dev @figma/plugin-typings

If you are familiar with JavaScript, TypeScript will look very familiar. In fact, valid JavaScript code
is already valid Typescript code.

TypeScript adds type annotations to variables. This allows code editors such as Visual Studio Code
to provide information about the Figma API while you are writing code, as well as help catch bugs
you previously didn't notice.

For more information, visit https://www.typescriptlang.org/

Using TypeScript requires a compiler to convert TypeScript (code.ts) into JavaScript (code.js)
for the browser to run.

We recommend writing TypeScript code using Visual Studio code:

1. Download Visual Studio Code if you haven't already: https://code.visualstudio.com/.
2. Open this directory in Visual Studio Code.
3. Compile TypeScript to JavaScript: Run the "Terminal > Run Build Task..." menu item,
    then select "npm: watch". You will have to do this again every time
    you reopen Visual Studio Code.

That's it! Visual Studio Code will regenerate the JavaScript file every time you save.
