// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { fileURLToPath } from "node:url";

export const previewFontPath = fileURLToPath(
    new URL(
        "../../../ui-libraries/material/examples/gallery/ui/fonts/Roboto-VariableFont.ttf",
        import.meta.url,
    ),
);
