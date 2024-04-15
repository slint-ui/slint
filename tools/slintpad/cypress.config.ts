// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Community OR LicenseRef-Slint-commercial

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
