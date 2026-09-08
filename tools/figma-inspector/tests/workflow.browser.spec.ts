// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { page, server } from "vitest/browser";
import JSZip from "jszip";
import { convertExport } from "../src/preview/convert-capture";
import { FIRST_BUTTON_SOURCE } from "../src/preview/sources";
import { mountPreview, readFixture } from "./browser-harness";
import type { Diagnostic } from "../src/plugin/snapshot";

test.each([true, false])(
    "diagnostics groups warnings across nodes (production=%s)",
    async (production) => {
        const p = await mountPreview(production);
        const warnings: Diagnostic[] = Array.from({ length: 30 }, (_, i) => ({
            severity: "warning",
            code: "BLEND_MODE_APPROXIMATED",
            message:
                "Non-normal blend modes are rendered with normal compositing",
            category: "approximation",
            nodeId: `node-${i}`,
            nodeName: `Repeated node ${i}`,
            nodePath: `Frame / Repeated node ${i}`,
            propertyPath: "blendMode",
            originalValue: i % 2 ? "SCREEN" : "MULTIPLY",
        }));
        warnings.push({
            severity: "warning",
            code: "SHADOW_BLEND_MODE_IGNORED",
            message: "Drop shadow blend mode is not supported and was ignored",
        });
        p.send({
            type: "preview-source",
            revision: 1,
            source: FIRST_BUTTON_SOURCE,
            warnings,
        });
        await p.ready(1);
        p.element("#diagnostics-tab").click();
        p.element<HTMLDetailsElement>("#diagnostics details").open = true;
        expect(p.doc.querySelectorAll("#diagnostics p")).toHaveLength(2);
        expect(p.element("#diagnostics summary").textContent).toBe(
            "2 approximations",
        );
        expect(p.element("#diagnostics").textContent).not.toMatch(
            /Repeated node|Original:|MULTIPLY|SCREEN/,
        );
        const screenshot = await page
            .elementLocator(p.iframe)
            .screenshot({ save: false });
        await server.commands.writeFile(
            `.vitest/attachments/grouped-warnings-${production ? "production" : "development"}.png`,
            screenshot,
            "base64",
        );

        // The map belongs to this presentation, so reuse and later selections
        // neither lose warnings nor retain obsolete warnings.
        p.send({
            type: "preview-source",
            revision: 2,
            source: FIRST_BUTTON_SOURCE,
            warnings,
        });
        await p.ready(2);
        expect(p.doc.querySelectorAll("#diagnostics p")).toHaveLength(2);
        p.send({
            type: "preview-source",
            revision: 3,
            source: FIRST_BUTTON_SOURCE,
            warnings: [warnings[0]],
        });
        await p.ready(3);
        expect(p.doc.querySelectorAll("#diagnostics p")).toHaveLength(1);
        expect(p.element("#diagnostics summary").textContent).toBe(
            "1 approximation",
        );
        p.send({
            type: "preview-source",
            revision: 4,
            source: FIRST_BUTTON_SOURCE,
            warnings: [],
        });
        await p.ready(4);
        expect(p.element("#diagnostics").hidden).toBe(true);
    },
);

test("real conversion worker supplies native source, raw clipboard text and the exported ZIP", async () => {
    const p = await mountPreview();
    const request = {
        type: "preview-capture" as const,
        revision: 1,
        captureJson: await readFixture("fixtures/source/export-fonts.json"),
        selection: { nodeId: "export:root", nodeName: "Export sample" },
    };
    const expected = await convertExport(request);
    p.send(request);
    await p.ready(1);
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toBe(expected.source);
    await expect
        .poll(() => p.element("#source-view pre.shiki").checkVisibility())
        .toBe(true);
    const copied: string[] = [];
    p.doc.execCommand = () => {
        copied.push((p.doc.activeElement as HTMLTextAreaElement).value);
        return true;
    };
    const copy = p.element<HTMLButtonElement>("#copy-button");
    copy.focus();
    copy.click();
    await expect.poll(() => copied).toEqual([expected.source]);
    expect(p.doc.activeElement).toBe(copy);
    expect(p.doc.querySelector("textarea")).toBeNull();
    p.doc.execCommand = () => false;
    copy.click();
    await expect
        .poll(() =>
            p.messages.some((m) => m.type === "clipboard-result" && !m.success),
        )
        .toBe(true);
    await expect
        .poll(() =>
            p.messages.some((m) => m.type === "clipboard-result" && m.success),
        )
        .toBe(true);
    // Intercept the download boundary only; generation, packaging and button wiring remain real.
    const realm = p.win as Window & typeof globalThis;
    let downloaded: Promise<ArrayBuffer> | undefined;
    let filename = "";
    const create = realm.URL.createObjectURL.bind(realm.URL);
    realm.URL.createObjectURL = (value: Blob | MediaSource) => {
        if (value instanceof realm.Blob && value.type === "application/zip")
            downloaded = value.arrayBuffer();
        return create(value);
    };
    realm.HTMLAnchorElement.prototype.click = function () {
        filename = this.download;
    };
    p.element<HTMLButtonElement>("#export-button").click();
    await expect.poll(() => downloaded !== undefined).toBe(true);
    if (!downloaded) throw Error("Missing exported ZIP");
    const zip = await JSZip.loadAsync(new Uint8Array(await downloaded));
    expect(filename).toBe("Export-sample.zip");
    expect(await zip.file("main.slint")?.async("string")).toBe(expected.source);
    p.send({ type: "preview-busy", revision: 2 });
    await expect
        .poll(() => (p.element("#export-button") as HTMLButtonElement).disabled)
        .toBe(true);
    await expect.poll(() => p.element("#source-view").textContent).toBe("");
});

test("pin controls send typed messages and diagnostics tabs support keyboard navigation", async () => {
    const p = await mountPreview();
    p.send({
        type: "preview-source",
        revision: 1,
        source: FIRST_BUTTON_SOURCE,
    });
    await p.ready(1);
    p.send({ type: "pin-state", pinned: false, canPin: true });
    const pin = p.element<HTMLButtonElement>("#pin-button");
    await expect.poll(() => (pin as HTMLButtonElement).disabled).toBe(false);
    pin.click();
    await expect
        .poll(() => p.messages.some((m) => m.type === "pin-selection"))
        .toBe(true);
    p.send({
        type: "pin-state",
        pinned: true,
        canPin: false,
        pinnedRoot: { nodeId: "root", nodeName: "Authored root" },
    });
    await expect.poll(() => pin.getAttribute("aria-pressed")).toBe("true");
    pin.click();
    await expect
        .poll(() => p.messages.some((m) => m.type === "unpin"))
        .toBe(true);
    const tab = p.element<HTMLButtonElement>("#preview-tab");
    tab.dispatchEvent(
        new (p.win as Window & typeof globalThis).KeyboardEvent("keydown", {
            key: "ArrowRight",
            bubbles: true,
        }),
    );
    await expect
        .poll(() => p.element("#diagnostics-tab").getAttribute("aria-selected"))
        .toBe("true");
    p.element("#diagnostics-tab").dispatchEvent(
        new (p.win as Window & typeof globalThis).KeyboardEvent("keydown", {
            key: "ArrowLeft",
            bubbles: true,
        }),
    );
    await expect
        .poll(() => p.element("#preview-tab").getAttribute("aria-selected"))
        .toBe("true");
});

test("highlighted source keeps long lines and the last line reachable", async () => {
    const p = await mountPreview();
    const source =
        FIRST_BUTTON_SOURCE +
        "\n" +
        Array.from(
            { length: 30 },
            (_, i) => `// line ${i} ${"x".repeat(150)}`,
        ).join("\n");
    p.send({ type: "preview-source", revision: 1, source });
    await p.ready(1);
    await expect.poll(() => p.element("#source-view").textContent).toBe(source);
    const view = p.element("#source-view");
    await expect
        .poll(() => view.querySelectorAll(".line").length)
        .toBe(source.split("\n").length);
    expect(view.scrollHeight).toBeGreaterThan(view.clientHeight);
    expect(view.scrollWidth).toBeGreaterThan(view.clientWidth);
    view.scrollTop = view.scrollHeight;
    view.scrollLeft = view.scrollWidth;
    const last = view.querySelector(".line:last-child");
    if (!last) throw Error("Missing last source line");
    expect(last.getBoundingClientRect().bottom).toBeLessThanOrEqual(
        view.getBoundingClientRect().bottom + 1,
    );
});
