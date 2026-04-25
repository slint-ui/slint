// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import fs from "node:fs";

export default {
    themes: ["dark-plus", "light-plus"],
    styleOverrides: {
        borderRadius: "0.4rem",
        borderColor: "var(--slint-code-background)",
        frames: { shadowColor: "transparent" },
        codeBackground: "var(--slint-code-background)",
    },
    shiki: {
        langs: [
            JSON.parse(
                fs.readFileSync(
                    "../common/src/utils/slint.tmLanguage.json",
                    "utf-8",
                ),
            ),
        ],
    },
};
