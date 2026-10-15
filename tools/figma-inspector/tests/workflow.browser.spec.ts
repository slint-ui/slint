// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { page, server } from "vitest/browser";
import JSZip from "jszip";
import { scalePreview } from "../src/preview/scale-preview";
import { convertSnapshot } from "../src/preview/converter";
import { convertCapture } from "../src/preview/convert-capture";
import {
    TimingTraceBuilder,
    type TimingTrace,
} from "../src/performance/timing";
import { CaptureTimingCollector } from "../src/performance/capture-timing";
import { packPreviewAssets } from "../src/asset-transport";
import {
    decodePng,
    mountCanvasPreview,
    expectCanvasPixels,
    mountPreview,
    readFixture,
} from "./browser-harness";
import type { Diagnostic } from "../src/plugin/snapshot";
import type { SourceCapture } from "../src/plugin/source";

const buttonSource = await readFixture("fixtures/button.slint");

test("packed image assets compile through preview object URLs", async () => {
    const p = await mountPreview();
    const image = await server.commands.readFile(
        "fixtures/authored/square.png",
        "base64",
    );
    const largeSvg = btoa(
        `<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="blue"/><!--${"x".repeat(140 * 1024)}--></svg>`,
    );
    const source = `export component MainWindow inherits Window {
        width: 64px;
        height: 32px;
        HorizontalLayout {
            Image { source: @image-url("data:image/png;base64,${image}"); }
            Image { source: @image-url("data:image/svg+xml;base64,${largeSvg}"); }
        }
    }`;
    const packed = packPreviewAssets(source);
    expect(packed.assets).toHaveLength(2);

    const exportSource = source
        .replace(`data:image/png;base64,${image}`, "assets/square.png")
        .replace(`data:image/svg+xml;base64,${largeSvg}`, "assets/large.svg");
    p.send({
        type: "preview-source",
        revision: 1,
        source: packed,
        exportPackage: {
            source: exportSource,
            files: [
                {
                    path: "assets/square.png",
                    data: image,
                    encoding: "base64",
                },
                {
                    path: "assets/large.svg",
                    data: largeSvg,
                    encoding: "base64",
                },
            ],
        },
    });

    await p.ready(1);
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toBe(exportSource);
    p.send({ type: "preview-source", revision: 2, source: packed });
    await p.ready(2);
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toContain("Copy Slint includes the complete source");
    expect(p.element<HTMLButtonElement>("#export-button").disabled).toBe(true);
    const copied: string[] = [];
    p.doc.execCommand = () => {
        copied.push((p.doc.activeElement as HTMLTextAreaElement).value);
        return true;
    };
    p.element<HTMLButtonElement>("#copy-button").click();
    await expect.poll(() => copied).toEqual([source]);
    expect(copied[0]).not.toContain("blob:");
});

test("large export image data validates without inline source expansion", async () => {
    const p = await mountPreview();
    p.send({
        type: "preview-source",
        revision: 1,
        source: buttonSource,
        exportPackage: {
            source: `${buttonSource}\n// @image-url("assets/large.png")`,
            files: [
                {
                    path: "assets/large.png",
                    data: "A".repeat(1024 * 1024),
                    encoding: "base64",
                },
            ],
        },
    });

    await p.ready(1);
    expect(p.element("#diagnostics").textContent).not.toContain(
        "EXPORT_VALIDATION_SKIPPED",
    );
    expect(p.element<HTMLButtonElement>("#export-button").disabled).toBe(false);
});

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
            source: buttonSource,
            warnings,
            exportPackage: { source: buttonSource, files: [] },
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
            source: buttonSource,
            warnings,
            exportPackage: { source: buttonSource, files: [] },
        });
        await p.ready(2);
        expect(p.doc.querySelectorAll("#diagnostics p")).toHaveLength(2);
        p.send({
            type: "preview-source",
            revision: 3,
            source: buttonSource,
            warnings: [warnings[0]],
            exportPackage: { source: buttonSource, files: [] },
        });
        await p.ready(3);
        expect(p.doc.querySelectorAll("#diagnostics p")).toHaveLength(1);
        expect(p.element("#diagnostics summary").textContent).toBe(
            "1 approximation",
        );
        p.send({
            type: "preview-source",
            revision: 4,
            source: buttonSource,
            warnings: [],
            exportPackage: { source: buttonSource, files: [] },
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
    const expected = await convertCapture(request).then((result) => {
        if (result.type !== "preview-source" || !result.exportPackage)
            throw Error(JSON.stringify(result));
        return result.exportPackage;
    });
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
    const copyMarkup = copy.innerHTML;
    copy.focus();
    copy.click();
    copy.click();
    await expect.poll(() => copied).toEqual([expected.source, expected.source]);
    expect(copy.innerHTML).toBe(copyMarkup);
    expect(copy.querySelector("svg")).not.toBeNull();
    await server.commands.writeFile(
        ".vitest/attachments/native-export.png",
        await page.elementLocator(p.iframe).screenshot({ save: false }),
        "base64",
    );
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
        source: buttonSource,
        exportPackage: { source: buttonSource, files: [] },
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

test("native variant failure shows the new specialized preview and disables export", async () => {
    const p = await mountPreview();
    p.send({
        type: "preview-source",
        revision: 1,
        source: buttonSource,
        exportPackage: { source: buttonSource, files: [] },
    });
    await p.ready(1);
    const capture = JSON.parse(
        await readFixture("fixtures/source/component-tokens.json"),
    );
    capture.root.children = capture.root.children.filter(
        (node: { id: string }) => node.id !== "hug:Pressed",
    );
    capture.variables.variables.pressed.values.light = {
        type: "VARIABLE_ALIAS",
        id: "pressed",
    };
    p.send({
        type: "preview-capture",
        revision: 2,
        captureJson: JSON.stringify(capture),
    });
    await p.ready(2);
    expect(p.element("#diagnostics").textContent).toContain(
        "Cyclic variable alias",
    );
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toContain("export component");
    expect(p.element("#source-view").textContent).not.toBe(buttonSource);
    expect(p.element<HTMLButtonElement>("#copy-button").disabled).toBe(false);
    expect(p.element<HTMLButtonElement>("#export-button").disabled).toBe(true);
    expect(
        p
            .element("#preview-canvas")
            .checkVisibility({ visibilityProperty: true }),
    ).toBe(true);
});

test("highlighted source keeps long lines and the last line reachable", async () => {
    const p = await mountPreview();
    const source =
        buttonSource +
        "\n" +
        Array.from(
            { length: 30 },
            (_, i) => `// line ${i} ${"x".repeat(150)}`,
        ).join("\n");
    p.send({
        type: "preview-source",
        revision: 1,
        source,
        exportPackage: { source: source, files: [] },
    });
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

test("large sections render their complete tree within the canvas limit", async () => {
    const snapshot = JSON.parse(
        await readFixture("fixtures/frame.snapshot.json"),
    );
    snapshot.root.width = 2561;
    snapshot.root.height = 14912;
    snapshot.root.fills = [
        { kind: "solid", color: { r: 1, g: 0, b: 0, a: 1 }, opacity: 1 },
    ];
    snapshot.root.children = [];
    snapshot.root.shadows = [];
    snapshot.root.opacity = 1;
    snapshot.root.strokes = [];
    const converted = convertSnapshot(
        scalePreview(snapshot, 2048 / snapshot.root.height),
    );
    expect(converted.ok).toBe(true);
    if (!converted.ok) return;
    const p = await mountPreview();
    p.send({ type: "preview-source", revision: 1, source: converted.source });
    await p.ready(1);
    const canvas = p.element<HTMLCanvasElement>("#preview-canvas");
    expect(canvas.height).toBeLessThanOrEqual(4096);
    expect(convertSnapshot(snapshot)).toMatchObject({
        ok: true,
        height: 14912,
    });
    const style = p.doc.createElement("style");
    const width = canvas.getBoundingClientRect().width;
    const height = canvas.getBoundingClientRect().height;
    p.doc.head.append(style);
    try {
        for (const fraction of [0.1, 0.9]) {
            style.textContent = `html,body {margin:0!important;overflow:hidden!important} body * {visibility:hidden!important} #preview-canvas {visibility:visible!important;position:fixed!important;left:0!important;top:${100 - height * fraction}px!important}`;
            await new Promise<void>((resolve) =>
                p.win.requestAnimationFrame(() => resolve()),
            );
            const pixels = await decodePng(
                await page.elementLocator(p.iframe).screenshot({ save: false }),
            );
            const x = Math.floor((width / 2) * p.win.devicePixelRatio);
            const y = Math.floor(100 * p.win.devicePixelRatio);
            const i = (y * pixels.width + x) * 4;
            expect([...pixels.data.slice(i, i + 3)]).toEqual([255, 0, 0]);
        }
    } finally {
        style.remove();
    }
    p.send({ type: "preview-source", revision: 2, source: buttonSource });
    await p.ready(2);
    expect(canvas.height).toBe(72 * p.win.devicePixelRatio);
});

test("late export timings update the finalized preview trace without extending its total", async () => {
    const p = await mountPreview(false);
    const builder = new TimingTraceBuilder(1, "initial");
    const collector = new CaptureTimingCollector();
    collector.measure("nodeReads", () => undefined);
    builder.setCaptureMetrics({
        durationMs: 0,
        requests: 0,
        exports: 0,
        cacheHits: 0,
        timings: collector.snapshot(),
    });
    p.send({
        type: "preview-source",
        revision: 1,
        source: buttonSource,
        trace: builder.toTrace(),
    });
    await p.ready(1);
    await expect
        .poll(() =>
            p.messages.find((message) => message.type === "preview-complete"),
        )
        .toBeDefined();
    const completed = p.messages.find(
        (message) => message.type === "preview-complete",
    );
    if (completed?.type !== "preview-complete")
        throw Error("No completed preview trace");
    p.send({ type: "preview-finalized", revision: 1, trace: completed.trace });
    p.send({
        type: "preview-export",
        revision: 1,
        warnings: [],
        exportPackage: { source: buttonSource, files: [] },
    });
    const debug = (
        p.win as unknown as {
            __slintPreviewDebug: { getLatestTrace(): TimingTrace };
        }
    ).__slintPreviewDebug;
    await expect
        .poll(() => debug.getLatestTrace()?.exportValidation?.outcome)
        .toBe("valid");
    expect(debug.getLatestTrace().totalMs).toBe(completed.trace.totalMs);
    expect(debug.getLatestTrace().breakdowns.presentation).toBeDefined();
    expect(
        p.element("[data-presentation-timing=firstFrameMs]").textContent,
    ).toContain("ms");
    expect(
        p.element("[data-presentation-timing=secondFrameMs]").textContent,
    ).toContain("ms");
    expect(p.element("[data-capture-timing=nodeReads]").textContent).toContain(
        "operations",
    );
    expect(
        p.element("[data-export-timing=compilationMs]").textContent,
    ).toContain("valid");
    expect(p.element<HTMLButtonElement>("#export-button").disabled).toBe(false);
});

test("repeat capture references reuse conversion and compiled preview without a full payload", async () => {
    const p = await mountPreview(false);
    const padding = " ".repeat(18 * 1024 * 1024);
    const source = JSON.parse(await readFixture("fixtures/source/basic.json"));
    p.send({
        type: "preview-capture",
        revision: 1,
        captureId: "A",
        captureJson: JSON.stringify(source) + padding,
        trace: new TimingTraceBuilder(1, "initial").toTrace(),
    });
    await p.ready(1);
    await expect
        .poll(() => p.element<HTMLButtonElement>("#export-button").disabled)
        .toBe(false);
    const changed = structuredClone(source);
    changed.root.properties.width += 20;
    p.send({
        type: "preview-capture",
        revision: 2,
        captureId: "B",
        captureJson: JSON.stringify(changed) + padding,
        trace: new TimingTraceBuilder(2, "selection-change").toTrace(),
    });
    await p.ready(2);
    await expect
        .poll(() => p.element<HTMLButtonElement>("#export-button").disabled)
        .toBe(false);
    p.send({ type: "preview-busy", revision: 3 });
    p.send({
        type: "preview-reuse",
        revision: 3,
        captureId: "A",
        trace: new TimingTraceBuilder(3, "selection-change").toTrace(),
    });
    await p.ready(3);
    await expect
        .poll(() =>
            p.messages.find(
                (message) =>
                    message.type === "preview-complete" &&
                    message.revision === 3,
            ),
        )
        .toBeDefined();
    const completed = p.messages.find(
        (message) =>
            message.type === "preview-complete" && message.revision === 3,
    );
    if (completed?.type !== "preview-complete")
        throw Error("Missing reused trace");
    expect(completed.trace.phases.slintCompilation).toBe(0);
    expect(completed.trace.phases.slintConversion).toBe(0);
    expect(completed.trace.phases.captureParse).toBe(0);
    expect(
        p.messages.some((message) => message.type === "capture-reuse-miss"),
    ).toBe(false);
    await expect
        .poll(() => p.element<HTMLButtonElement>("#export-button").disabled)
        .toBe(false);
});

test("transparent vector assets preserve layout and show the background beneath them", async () => {
    const p = await mountCanvasPreview();
    const packed =
        packPreviewAssets(`export component Demo inherits Window { width: 40px; height: 40px; background: red;
        Image { x: 10px; y: 10px; width: 20px; height: 20px; source: @image-url("data:image/svg+xml;base64,${btoa('<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"/>')}"); }
    }`);
    p.send({ type: "preview-source", revision: 1, source: packed });
    await p.ready(1);
    await expectCanvasPixels(p, (pixels) => {
        const offset = (20 * pixels.width + 20) * 4;
        expect(Array.from(pixels.data.slice(offset, offset + 4))).toEqual([
            255, 0, 0, 255,
        ]);
    });
});

test("streamed bulk metadata and live supplements render and validate through the real worker", async () => {
    const p = await mountPreview();
    const source = JSON.parse(
        await readFixture("fixtures/source/basic.json"),
    ) as SourceCapture;
    const expected = await convertCapture({
        type: "preview-capture",
        revision: 1,
        captureJson: JSON.stringify(source),
    });
    if (expected.type !== "preview-source" || !expected.exportPackage)
        throw Error("Missing native export");
    const document = {
        id: source.root.id,
        type: source.root.type,
        ...source.root.properties,
    };
    const keys = Object.keys(source.root.properties).filter(
        (key) => key !== "rotation",
    );
    source.bulkDocuments = [document];
    source.bulkPropertySets = [keys];
    source.root.bulkPropertySet = 0;
    source.root.properties = { rotation: source.root.properties.rotation };
    p.send({ type: "preview-busy", revision: 1 });
    p.send({ type: "preview-bulk", revision: 1, document });
    p.send({
        type: "preview-capture",
        revision: 1,
        captureId: "bulk",
        captureJson: JSON.stringify(source),
        trace: new TimingTraceBuilder(1, "initial").toTrace(),
    });
    await p.ready(1);
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toBe(expected.exportPackage.source);
    await expect
        .poll(() => (p.element("#export-button") as HTMLButtonElement).disabled)
        .toBe(false);
    expect(p.element("#diagnostics").hidden).toBe(true);
});
