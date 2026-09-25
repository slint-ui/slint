// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { AssetPreview } from "../src/asset-transport";

export function previewAssetSource(value: AssetPreview): string {
    return value.source
        .map((part) => (typeof part === "string" ? part : value.assets[part]))
        .join("");
}
