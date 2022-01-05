// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

import { defineConfig } from 'vite'
export default defineConfig({
  server: {
    fs: {
      // Allow serving files from the project root
      allow: ['../../']
    }
  },
  base: ''
})
