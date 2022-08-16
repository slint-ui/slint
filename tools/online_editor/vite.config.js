// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { defineConfig } from "vite";
export default defineConfig(({ command, _mode }) => {
  const base_config = {
    server: {
      fs: {
        // Allow serving files from the project root
        allow: ["../../"],
      },
    },
    base: "",
  };

  if (command === "serve") {
    // For development builds, serve the wasm interpreter straight out of the local file system.
    base_config.resolve = {
      alias: {
        "@preview/": "../../api/wasm-interpreter/pkg/",
      },
    };
  } else {
    // For distribution builds,
    // assume deployment on the main website where the loading file (index.js) is in the assets/ sub-directory and the
    // relative path to the interpreter is as below.
    base_config.build = {};
    base_config.build.rollupOptions = {
      external: ["../../../wasm-interpreter/slint_wasm_interpreter.js"],
      input: ["index.html", "preview.html"],
    };
    base_config.resolve = {
      alias: {
        "@preview/": "../../../wasm-interpreter/",
      },
    };
  }

  return base_config;
});
