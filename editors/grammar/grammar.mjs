// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import words from "./words.json" with { type: "json" };
import template from "./slint.tmLanguage.template.json" with { type: "json" };

/** Returns `text` with each `@NAME@` replaced by the `name` list of words.json, joined with `|`. */
export function fillTemplate(text) {
    return text.replace(/@([A-Z]+)@/g, (_, name) =>
        words[name.toLowerCase()].join("|"),
    );
}

/** The TextMate grammar for Slint. */
export const textMateGrammar = JSON.parse(
    fillTemplate(JSON.stringify(template)),
);
