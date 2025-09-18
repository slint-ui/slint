// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { Facet } from "@codemirror/state";

// Define a custom facet to hold the language name
export const languageNameFacet = Facet.define({
    combine(languages) {
        return languages.length ? languages[0] : null; // Combine to get the first language if set
    },
});
