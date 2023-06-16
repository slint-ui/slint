// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

// cSpell: ignore frsource

import "@frsource/cypress-plugin-visual-regression-diff/dist/support";

module.exports = (on, _config) => {
    on("before:browser:launch", (browser = {}, args: string[]) => {
        if (browser.name === "chromium" || browser.name == "chrome") {
            const newArgs = args.filter((arg) => arg !== "--disable-gpu");
            newArgs.push("--ignore-gpu-blacklist");
            return newArgs;
        }
    });
};
