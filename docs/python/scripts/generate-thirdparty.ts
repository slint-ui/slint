// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { generateThirdPartyMarkdown } from "@slint/common-files/src/utils/thirdparty.ts";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const docsPythonRoot = join(scriptsDir, "..");
const repoRoot = join(docsPythonRoot, "..", "..");

generateThirdPartyMarkdown({
    crateDir: join(repoRoot, "api", "python", "slint"),
    outFile: join(
        docsPythonRoot,
        "src",
        "content",
        "docs",
        "generated",
        "thirdparty.md",
    ),
});
