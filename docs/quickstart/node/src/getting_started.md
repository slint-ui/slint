<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Getting Started

This tutorial uses JavaScript as the host programming language. Slint also supports other programming languages like
[Rust](https://slint.dev/docs/rust/slint/) or [C++](https://slint.dev/docs/cpp/).

We recommend using [our editor integrations for Slint](https://github.com/slint-ui/slint/tree/master/editors) for following this tutorial.

Slint has an application template you can use to create a project with dependencies already set up that follows recommended best practices.

Clone the template with the following command:

```sh
git clone https://github.com/slint-ui/slint-nodejs-template memory
cd memory
```

Install dependencies with npm:

```sh
npm install
```

The `package.json` file references `src/main.js` as the entry point for the application and `src/main.js` references `memory.slint` as the UI file.

Replace the contents of `src/main.js` with the following:

```js
{{#include main_initial.js:main}}
```

The `slint.loadFile` method resolves files from the process's current working directory, so from the `package.json` file's location.

Replace the contents of `ui/appwindow.slint` with the following:

```slint
{{#include memory.slint:main_window}}
```

Run the example with `npm start` and a window appears with the green "Hello World" greeting.

![Screenshot of an initial tutorial app showing Hello World](https://slint.dev/blog/memory-game-tutorial/getting-started.png "Hello World")
