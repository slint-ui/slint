// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { fillTemplate, textMateGrammar } from "./grammar.mjs";

const dist = new URL("dist/", import.meta.url);
mkdirSync(dist, { recursive: true });

writeFileSync(
    new URL("slint.tmLanguage.json", dist),
    `${JSON.stringify(textMateGrammar, null, 4)}\n`,
);
writeFileSync(
    new URL("highlightjs-slint.js", dist),
    fillTemplate(
        readFileSync(
            new URL("highlightjs-slint.template.js", import.meta.url),
            "utf8",
        ),
    ),
);
