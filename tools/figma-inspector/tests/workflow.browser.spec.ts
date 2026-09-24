// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { page, server } from "vitest/browser";
import JSZip from "jszip";
import { convertCapture } from "../src/preview/convert-capture";
import { packPreviewAssets } from "../src/asset-transport";
import { mountPreview, readFixture } from "./browser-harness";
import type { Diagnostic } from "../src/plugin/snapshot";
import type { SourceCapture, SourceNode } from "../src/plugin/source";

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
});

test("oversized export validation cannot block the specialized preview", async () => {
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
    expect(p.element("#diagnostics").textContent).toContain(
        "EXPORT_VALIDATION_SKIPPED",
    );
});

test("flattened native-text capture compiles and retains its fidelity warning", async () => {
    const capture = JSON.parse(
        await readFixture("fixtures/source/export-fonts.json"),
    ) as SourceCapture;
    capture.flattened = true;
    delete capture.components;
    const stripTextExports = (node: SourceNode): void => {
        if (node.type === "TEXT") delete node.exports;
        node.children?.forEach(stripTextExports);
    };
    stripTextExports(capture.root);
    const warning: Diagnostic = {
        severity: "warning",
        code: "SIMPLIFIED_CAPTURE",
        category: "omission",
        message: "This is a simplified flattened export",
    };
    const converted = await convertCapture({
        type: "preview-capture",
        revision: 1,
        captureJson: JSON.stringify(capture),
        warnings: [warning],
    });
    expect(converted.type).toBe("preview-source");
    if (converted.type !== "preview-source") return;
    expect(converted.warnings).toContainEqual(warning);

    const p = await mountPreview();
    p.send(converted);
    await p.ready(1);
    expect(p.element("#diagnostics").textContent).toContain(
        "SIMPLIFIED_CAPTURE",
    );
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toContain("Text {");
});

test("capture timeout opens a concise dialog while the spinner continues", async () => {
    const p = await mountPreview();
    p.send({ type: "preview-busy", revision: 1 });
    const dialog = p.element<HTMLDialogElement>("#simplified-dialog");
    expect(dialog.open).toBe(false);
    p.send({
        type: "preview-busy",
        revision: 1,
        message: "This selection is large. Trying a simpler export.",
    });
    await expect.poll(() => dialog.open).toBe(true);
    expect(p.element("#simplified-dialog-message").textContent).toBe(
        "This selection is large. Trying a simpler export.",
    );
    expect(p.element("#preview-busy").hidden).toBe(false);
    p.element<HTMLButtonElement>("#simplified-dialog-ok").click();
    await expect.poll(() => dialog.open).toBe(false);
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
        if (result.type !== "preview-source")
            throw Error(JSON.stringify(result));
        return result.exportPackage;
    });
    p.send(request);
    await p.ready(1);
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toBe(expected.source);
    await expect
        .poll(() => p.element("#source-view pre.twinkleplop").checkVisibility())
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

test("native variant failure blanks the previous preview and disables all output", async () => {
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
    await expect.poll(() => p.element("#status").dataset.state).toBe("error");
    expect(p.element("#diagnostics").textContent).toContain(
        "Cyclic variable alias",
    );
    expect(p.element("#diagnostics-tab").getAttribute("aria-selected")).toBe(
        "true",
    );
    expect(p.element("#source-view").textContent).toBe("");
    expect(p.element<HTMLButtonElement>("#copy-button").disabled).toBe(true);
    expect(p.element<HTMLButtonElement>("#export-button").disabled).toBe(true);
    expect(
        p
            .element("#preview-canvas")
            .checkVisibility({ visibilityProperty: true }),
    ).toBe(false);
});

test("highlighted source keeps long lines and the last line reachable", async () => {
    const p = await mountPreview();
    const source =
        buttonSource +
        "\n" +
        Array.from(
            { length: 2_100 },
            (_, i) => `// line ${i} ${"x".repeat(i === 1_000 ? 2_100 : 40)}`,
        ).join("\n");
    expect(source.length).toBeGreaterThan(100_000);
    expect(source.split("\n").length).toBeGreaterThan(2_000);
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
        .poll(() => view.querySelectorAll(".l").length)
        .toBe(source.split("\n").length);
    expect(view.scrollHeight).toBeGreaterThan(view.clientHeight);
    expect(view.scrollWidth).toBeGreaterThan(view.clientWidth);
    view.scrollTop = view.scrollHeight;
    view.scrollLeft = view.scrollWidth;
    const last = view.querySelector(".l:last-child");
    if (!last) throw Error("Missing last source line");
    expect(last.getBoundingClientRect().bottom).toBeLessThanOrEqual(
        view.getBoundingClientRect().bottom + 1,
    );
});

test("highlighted Slint uses the matching colors in both themes", async () => {
    const p = await mountPreview();
    p.send({
        type: "preview-source",
        revision: 1,
        source: buttonSource,
        exportPackage: { source: buttonSource, files: [] },
    });
    await p.ready(1);
    const number = () =>
        Array.from(p.doc.querySelectorAll<HTMLElement>(".tok.number")).find(
            (token) => token.textContent === "240px",
        );
    await expect.poll(() => number() !== undefined).toBe(true);
    expect(p.element("#source-view pre.twinkleplop.light-slint")).toBeTruthy();
    expect(p.win.getComputedStyle(number() as HTMLElement).color).toBe(
        "rgb(234, 15, 172)",
    );

    p.element("#source-view pre.twinkleplop").classList.replace(
        "light-slint",
        "dark-slint",
    );
    expect(p.win.getComputedStyle(number() as HTMLElement).color).toBe(
        "rgb(252, 155, 223)",
    );
});
