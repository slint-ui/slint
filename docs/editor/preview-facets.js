// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { Facet } from '@codemirror/state';

export const previewFacet =  Facet.define({
    combine(wasm) {
        return wasm.length ? wasm[0] : undefined;
    }
});
