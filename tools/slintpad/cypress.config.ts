// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore frsource

import { defineConfig } from "cypress";
import { initPlugin } from "@frsource/cypress-plugin-visual-regression-diff/dist/plugins";

export default defineConfig({
    e2e: {
        baseUrl: "http://localhost:3001",
        setupNodeEvents(on, config) {
            initPlugin(on, config);
        },
    },
});
