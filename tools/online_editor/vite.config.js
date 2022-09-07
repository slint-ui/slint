// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// TODO: Do build and package wasm-lsp separately. Right now vite does not
// support `exclude` in web workers!

import { defineConfig } from "vite";

export default defineConfig(({ command, _mode }) => {
  const base_config = {
    server: {
      fs: {
        // Allow serving files from the project root
        allow: ["../../"],
      },
    },
    base: "./",
  };

  let global_aliases = {
    "@lsp/": "../../../lsp/pkg/",
    "~@lumino": "node_modules/@lumino/", // work around strange defaults in @lumino
  };

  if (command === "serve") {
    // For development builds, serve the wasm interpreter straight out of the local file system.
    base_config.resolve = {
      alias: {
        "@preview/": "../../../api/wasm-interpreter/pkg/",
        ...global_aliases,
      },
    };
  } else {
    // For distribution builds,
    // assume deployment on the main website where the loading file (index.js) is in the assets/ sub-directory and the
    // relative path to the interpreter is as below.
    base_config.build = {};
    base_config.build.rollupOptions = {
      external: ["../../../../wasm-interpreter/slint_wasm_interpreter.js"],
      input: ["index.html", "preview.html"],
    };
    base_config.resolve = {
      alias: {
        "@preview/": "../../../../wasm-interpreter/",
        ...global_aliases,
      },
    };
  }

  return base_config;
});
