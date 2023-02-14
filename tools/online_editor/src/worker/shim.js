// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// UGLY HACK: Vite does produce invalid code when packaging a web worker as
// iife, see https://github.com/vitejs/vite/issues/9879
//
// We need to produce iife, otherwise firefox will not be supported :-/
//
// The issue is that the generated code uses `document` which does not exist
// on a web worker. So we just add one with all the needed values for this to
// work and make sure it gets imported before its needed...

let assets = new URL("assets/", location.origin);

self.document = {
    baseURI: assets,
};
