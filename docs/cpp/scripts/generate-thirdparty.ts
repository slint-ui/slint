// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { generateThirdPartyMarkdown } from "@slint/common-files/src/utils/thirdparty.ts";

const scriptsDir = dirname(fileURLToPath(import.meta.url));
const docsCppRoot = join(scriptsDir, "..");
const repoRoot = join(docsCppRoot, "..", "..");

generateThirdPartyMarkdown({
    crateDir: join(repoRoot, "api", "cpp"),
    outFile: join(
        docsCppRoot,
        "src",
        "content",
        "docs",
        "generated",
        "thirdparty.md",
    ),
});
