// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { generateSlintSnippet } from "./utils/property-parsing.js";

if (figma.editorType === "dev" && figma.mode === "codegen") {
    figma.codegen.on("generate", async ({ node }) => {
        const slintSnippet = await generateSlintSnippet(node);
        return [
            {
                title: "Slint Code: " + node.name,
                language: "CSS",
                code: slintSnippet,
            },
        ];
    });
}
