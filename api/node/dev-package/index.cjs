// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// slint-ui-dev only provides the development native binary for slint-ui; it is
// not meant to be imported directly. The actual binary is exposed via the
// "slint-ui-dev/loader" subpath, which slint-ui loads internally. Importing this
// package directly is almost always a mistake, so fail loudly with a hint.

"use strict";

throw new Error(
    "slint-ui-dev must not be imported directly — it only provides the " +
        'development binary for slint-ui. Import from "slint-ui" instead; having ' +
        "slint-ui-dev installed as a devDependency is enough to enable the " +
        "development binary.",
);
