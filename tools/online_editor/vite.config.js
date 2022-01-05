// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

import { defineConfig } from 'vite'
export default defineConfig(({ command, mode }) => {
  let base_config = {
    server: {
      fs: {
        // Allow serving files from the project root
        allow: ['../../']
      }
    },
    base: '',
  };

  if (command === "serve") {
    // For development builds, serve the wasm interpreter straight out of the local file system.
    base_config.resolve = {
      alias: {
        '../../../wasm-interpreter/sixtyfps_wasm_interpreter.js': "../../api/sixtyfps-wasm-interpreter/pkg/sixtyfps_wasm_interpreter.js"
      }
    }
  } else {
    // For distribution builds,
    // assume deployment on the main website where the loading file (index.js) is in the assets/ sub-directory and the
    // relative path to the interpreter is as below.
    base_config.build = {}
    base_config.build.rollupOptions = {
      external: ['../../../wasm-interpreter/sixtyfps_wasm_interpreter.js'],
      input: ["index.html", "preview.html"],
    }
  }

  return base_config;
})
