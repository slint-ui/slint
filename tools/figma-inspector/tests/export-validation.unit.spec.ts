// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { prepareExportValidation } from "../src/preview/export-validation";
import { createPreviewAssetPool } from "../src/asset-transport";

test("large image payloads stay outside compiler source and blob lifetime is explicit", () => {
    const revoked: string[] = [];
    const pool = createPreviewAssetPool({
        createObjectUrl: () => "blob:export",
        revokeObjectUrl: (url) => revoked.push(url),
    });
    const data = btoa(
        `<svg xmlns="http://www.w3.org/2000/svg"><!--${"x".repeat(2 * 1024 * 1024)}--></svg>`,
    );
    const result = prepareExportValidation(
        {
            source: 'export component Demo inherits Window { Image { source: @image-url("assets/large.svg"); } Text { text: "assets/large.svg"; } }',
            files: [{ path: "assets/large.svg", data, encoding: "base64" }],
        },
        pool,
    );
    expect(result.source).toContain('@image-url("blob:export")');
    expect(result.source).toContain('text: "assets/large.svg"');
    expect(result.source.length).toBeLessThan(200);
    expect(revoked).toEqual([]);
    result.dispose();
    result.dispose();
    expect(revoked).toEqual(["blob:export"]);
});

test("missing external export images fail instead of validating an incomplete project", () => {
    expect(() =>
        prepareExportValidation(
            {
                source: 'Image { source: @image-url("assets/missing.png"); }',
                files: [],
            },
            createPreviewAssetPool(),
        ),
    ).toThrow("Missing export image");
});
