// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { startPreview } from "./preview-main";
import { generateCodegen } from "./codegen";

if (figma.editorType === "dev" && figma.mode === "codegen") {
    figma.codegen.on("generate", ({ node }) =>
        generateCodegen(node, figma.mixed),
    );
} else {
    startPreview();
}
