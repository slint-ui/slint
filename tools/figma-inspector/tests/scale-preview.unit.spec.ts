// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import { scalePreview } from "../src/preview/scale-preview";
import { validateSnapshot } from "../src/plugin/snapshot";
import { previewAssetSource } from "./preview-asset-source";
import {
    convertCapture,
    captureSnapshotJson,
} from "../src/preview/convert-capture";

test("preview scaling preserves colors, font weights, asset pixels, and original snapshots", async () => {
    const snapshot = JSON.parse(
        await readFile("fixtures/frame.snapshot.json", "utf8"),
    );
    const original = JSON.stringify(snapshot);
    const scaled = scalePreview(snapshot, 0.5);
    expect(validateSnapshot(scaled).ok).toBe(true);
    expect(scaled.root.width).toBe(120);
    expect(scaled.root.opacity).toBe(snapshot.root.opacity);
    if (!("fills" in scaled.root) || !("children" in scaled.root))
        throw Error("Missing scaled frame");
    expect(scaled.root.fills).toEqual(snapshot.root.fills);
    expect(scaled.root.cornerRadii).toEqual(
        snapshot.root.cornerRadii.map((radius: number) => radius * 0.5),
    );
    const text = scaled.root.children[0];
    if (text.kind !== "text") throw Error("Missing scaled text");
    expect(text.fontSize).toBe(snapshot.root.children[0].fontSize * 0.5);
    expect(text.fontWeight).toBe(snapshot.root.children[0].fontWeight);
    expect(JSON.stringify(snapshot)).toBe(original);
});

test("large captures keep full-size snapshots and exports while bounding preview geometry", async () => {
    const source = JSON.parse(
        await readFile("fixtures/source/basic.json", "utf8"),
    );
    source.root.properties.width = 2561;
    source.root.properties.height = 14912;
    const state = {};
    const result = await convertCapture(
        {
            type: "preview-capture",
            revision: 1,
            captureJson: JSON.stringify(source),
        },
        state,
    );
    expect(result.type).toBe("preview-source");
    if (result.type !== "preview-source") throw Error("Missing preview");
    expect(result.exportPackage?.source).toContain("height: 14912px;");
    const preview =
        typeof result.source === "string"
            ? result.source
            : previewAssetSource(result.source);
    expect(preview).toContain("height: 2048px;");
    expect(JSON.parse(captureSnapshotJson(state)).root.height).toBe(14912);
});
