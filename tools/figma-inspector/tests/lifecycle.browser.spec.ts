// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import { page, server } from "vitest/browser";
import { mountPreview, canvasPixels, readFixture } from "./browser-harness";

const firstSource = await readFixture("fixtures/button.slint");
const secondSource = await readFixture("fixtures/frame.slint");

test.each([false, true])(
    "a valid preview survives an invalid reusable export (unchanged preview: %s)",
    async (warm) => {
        const p = await mountPreview(false);
        if (warm) {
            p.send({
                type: "preview-source",
                revision: 1,
                source: firstSource,
                exportPackage: { source: firstSource, files: [] },
            });
            await p.ready(1);
        }
        p.send({
            type: "preview-source",
            revision: 2,
            source: firstSource,
            exportPackage: {
                source: "export component Broken inherits Window { Text { visible: missing.preferred-height > 0px; } }",
                files: [],
            },
        });
        await expect
            .poll(() => p.element("#diagnostics").textContent)
            .toContain("Export compilation failed");
        expect(p.element<HTMLButtonElement>("#export-button").disabled).toBe(
            true,
        );
        await p.ready(2);
        expect(p.element("#diagnostics summary").textContent).toBe(
            "1 export failure",
        );
        expect(p.element("#diagnostics").textContent).toContain(
            "Export ZIP is disabled",
        );
        expect(p.element<HTMLButtonElement>("#copy-button").disabled).toBe(
            false,
        );
        await expect
            .poll(() => p.element("#source-view").textContent)
            .toBe(firstSource);
        expect(p.element("#preview-canvas").style.visibility).toBe("visible");
        await expect
            .poll(() =>
                p.messages.some(
                    (message) =>
                        message.type === "preview-complete" &&
                        message.revision === 2,
                ),
            )
            .toBe(true);
        const completion = p.messages.find(
            (message) =>
                message.type === "preview-complete" && message.revision === 2,
        );
        if (completion?.type !== "preview-complete")
            throw Error("Missing completion trace");
        p.send({
            type: "preview-finalized",
            revision: 2,
            trace: completion.trace,
        });
        const lineCount = firstSource.trimEnd().split(/\r\n|\r|\n/u).length;
        await expect
            .poll(
                () =>
                    p.doc.querySelector("[data-generated-source-lines]")
                        ?.textContent,
            )
            .toBe(`${lineCount} lines`);
        if (!warm) {
            await server.commands.writeFile(
                ".vitest/attachments/export-failure-preview.png",
                await page.elementLocator(p.iframe).screenshot({ save: false }),
                "base64",
            );
            p.element("#diagnostics-tab").click();
            p.element<HTMLDetailsElement>("#diagnostics details").open = true;
            await server.commands.writeFile(
                ".vitest/attachments/export-failure-diagnostics.png",
                await page.elementLocator(p.iframe).screenshot({ save: false }),
                "base64",
            );
            p.element("#preview-tab").click();
        }
        p.send({
            type: "preview-source",
            revision: 3,
            source: firstSource,
            exportPackage: { source: firstSource, files: [] },
        });
        await p.ready(3);
        expect(p.element<HTMLButtonElement>("#export-button").disabled).toBe(
            false,
        );
    },
);

test("built production preview clears busy, failed and empty selections and rejects stale output", async () => {
    const p = await mountPreview();
    p.send({
        type: "preview-source",
        revision: 1,
        source: firstSource,
        exportPackage: { source: firstSource, files: [] },
    });
    await p.ready(1);
    expect(p.doc.querySelector("#timing-panel")).toBeNull();
    expect(p.doc.querySelector("#copy-snapshot")).toBeNull();
    const pixels = await canvasPixels(p);
    expect(pixels.width).toBe(240);
    expect(pixels.height).toBe(72);
    expect(pixels.data.some((v, i) => i % 4 !== 3 && v < 200)).toBe(true);
    p.send({ type: "preview-busy", revision: 2 });
    await expect
        .poll(() =>
            p
                .element("#preview-canvas")
                .checkVisibility({ visibilityProperty: true }),
        )
        .toBe(false);
    await expect.poll(() => p.element("#source-view").textContent).toBe("");
    await expect
        .poll(() => (p.element("#export-button") as HTMLButtonElement).disabled)
        .toBe(true);
    p.send({
        type: "preview-diagnostics",
        revision: 2,
        diagnostics: [
            { severity: "error", code: "TEST", message: "Selection failed" },
        ],
    });
    await expect
        .poll(() => p.element("#diagnostics-tab").getAttribute("aria-selected"))
        .toBe("true");
    await expect
        .poll(() => p.element("#diagnostics").textContent)
        .toContain("Selection failed");
    p.send({
        type: "preview-source",
        revision: 1,
        source: firstSource,
        exportPackage: { source: firstSource, files: [] },
    });
    p.send({
        type: "preview-source",
        revision: 3,
        source: secondSource,
        exportPackage: { source: secondSource, files: [] },
    });
    await p.ready(3);
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toBe(secondSource);
    p.send({ type: "preview-clear", revision: 4 });
    await expect
        .poll(() =>
            p
                .element("#preview-canvas")
                .checkVisibility({ visibilityProperty: true }),
        )
        .toBe(false);
    await expect
        .poll(() =>
            p
                .element("#diagnostics")
                .checkVisibility({ visibilityProperty: true }),
        )
        .toBe(false);
    await expect.poll(() => p.element("#source-view").textContent).toBe("");
    await expect
        .poll(() => (p.element("#export-button") as HTMLButtonElement).disabled)
        .toBe(true);
});

test("rapid revisions settle on the newest source and recover after compiler errors", async () => {
    const p = await mountPreview();
    for (let revision = 1; revision <= 12; revision++)
        p.send({
            type: "preview-source",
            revision,
            source: `// ${revision}\n${firstSource}`,
            exportPackage: {
                source: `// ${revision}\n${firstSource}`,
                files: [],
            },
        });
    await p.ready(12);
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toContain("// 12");
    p.send({
        type: "preview-source",
        revision: 13,
        source: "not valid Slint",
        exportPackage: { source: "not valid Slint", files: [] },
    });
    await expect
        .poll(() => p.element("#diagnostics-tab").getAttribute("aria-selected"))
        .toBe("true");
    await expect
        .poll(() =>
            p
                .element("#preview-canvas")
                .checkVisibility({ visibilityProperty: true }),
        )
        .toBe(false);
    p.send({
        type: "preview-source",
        revision: 14,
        source: firstSource,
        exportPackage: { source: firstSource, files: [] },
    });
    await p.ready(14);
    p.element("#preview-tab").click();
    await expect
        .poll(() =>
            p
                .element("#preview-canvas")
                .checkVisibility({ visibilityProperty: true }),
        )
        .toBe(true);
});
