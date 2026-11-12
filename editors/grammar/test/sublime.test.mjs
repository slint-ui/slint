// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// The Sublime grammar is installed from the repository, so it's maintained by hand.
// Checks that its patterns list the words of words.json.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import words from "../words.json" with { type: "json" };

const patterns = readFileSync(
    new URL("../../sublime/Slint.sublime-syntax", import.meta.url),
    "utf8",
)
    .split("\n")
    .filter((line) => line.trim().startsWith("- match:"))
    .join("\n");

for (const list of ["declarations", "keywords", "controls", "types"]) {
    test(list, () => {
        const missing = words[list].filter(
            (word) => !new RegExp(`[(|]${word}[|)]`).test(patterns),
        );
        assert.deepEqual(missing, []);
    });
}
