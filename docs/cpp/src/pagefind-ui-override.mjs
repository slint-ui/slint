// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cSpell:ignore Pagefind

// Wrap Pagefind's `PagefindUI` to inject a `processTerm` that normalizes C++
// scope/identifier separators (`::`, `_`) to spaces, so a search for a qualified
// name matches (Pagefind splits those apart when indexing but keeps them joined
// in a query). Aliased in for `@pagefind/default-ui` (see astro.config.mjs), so
// Starlight's `new PagefindUI(...)` picks it up. Imports the dependency by its
// concrete path to avoid the alias recursing into this file.
import { PagefindUI as Base } from "@pagefind/default-ui/npm_dist/mjs/ui-core.mjs";

export class PagefindUI extends Base {
    constructor(opts = {}) {
        super({ processTerm: (t) => t.replace(/[:_]+/g, " "), ...opts });
    }
}
