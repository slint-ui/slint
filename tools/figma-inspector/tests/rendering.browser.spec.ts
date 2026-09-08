// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { test } from "vitest";
import { convertSnapshotJson, convertSnapshot } from "../src/preview/converter";
import { normalizeSource } from "../src/plugin/normalize";
import { mountPreview, readFixture } from "./browser-harness";

test("authored layouts, images and component families compile through the built interpreter", async () => {
    const p = await mountPreview();
    let revision = 0;
    for (const name of [
        "button",
        "auto-layout",
        "hug-button",
        "nested-fill",
        "nested-instance",
        "image-fill",
        "svg-multi-path",
    ]) {
        const result = convertSnapshotJson(
            await readFixture(`fixtures/${name}.snapshot.json`),
        );
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({
            type: "preview-source",
            revision: ++revision,
            source: result.source,
        });
        await p.ready(revision);
    }
    for (const name of [
        "component-intrinsic",
        "component-variants",
        "component-tokens",
        "empty-flex-spacers",
        "group-positioning",
        "image-crop",
        "painted-bounds",
    ]) {
        const normalized = await normalizeSource(
            JSON.parse(await readFixture(`fixtures/source/${name}.json`)),
        );
        if (!normalized.ok || normalized.empty)
            throw Error(`Cannot normalize ${name}`);
        const result = convertSnapshot(normalized.snapshot);
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({
            type: "preview-source",
            revision: ++revision,
            source: result.source,
        });
        await p.ready(revision);
    }
});

test("root-only snippets compile with native text and appearance helpers without font files", async () => {
    const p = await mountPreview();
    let revision = 0;
    for (const name of [
        "button",
        "frame",
        "auto-layout",
        "nested-instance",
        "image-fill",
        "svg-multi-path",
        "component-text",
    ]) {
        const snapshot = JSON.parse(
            await readFixture(`fixtures/${name}.snapshot.json`),
        );
        const result = convertSnapshot(snapshot, { scope: "root-only" });
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({
            type: "preview-source",
            revision: ++revision,
            source: `export component Snippet inherits Window { width: 640px; height: 480px; ${result.source} }`,
        });
        await p.ready(revision);
    }
    for (const name of [
        "translucent-styled-text",
        "inner-shadow",
        "asymmetric-rounded-stroke",
        "reverse-paint-order",
    ]) {
        const source = JSON.parse(
            await readFixture(`fixtures/source/${name}.json`),
        );
        if (name === "translucent-styled-text")
            source.root = source.root.children[0];
        const normalized = await normalizeSource(source, "export");
        if (!normalized.ok || normalized.empty)
            throw Error(`Cannot normalize ${name}`);
        const result = convertSnapshot(normalized.snapshot, {
            scope: "root-only",
        });
        if (!result.ok) throw Error(JSON.stringify(result.diagnostics));
        p.send({
            type: "preview-source",
            revision: ++revision,
            source: `export component Snippet inherits Window { width: 640px; height: 480px; ${result.source} }`,
        });
        await p.ready(revision);
    }
});
