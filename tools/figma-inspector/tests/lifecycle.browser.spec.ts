// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test } from "vitest";
import {
    FIRST_BUTTON_SOURCE,
    SECOND_BUTTON_SOURCE,
} from "../src/preview/sources";
import { mountPreview, canvasPixels } from "./browser-harness";

test("built production preview clears busy, failed and empty selections and rejects stale output", async () => {
    const p = await mountPreview();
    p.send({
        type: "preview-source",
        revision: 1,
        source: FIRST_BUTTON_SOURCE,
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
        source: FIRST_BUTTON_SOURCE,
    });
    p.send({
        type: "preview-source",
        revision: 3,
        source: SECOND_BUTTON_SOURCE,
    });
    await p.ready(3);
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toBe(SECOND_BUTTON_SOURCE);
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
            source: `// ${revision}\n${FIRST_BUTTON_SOURCE}`,
        });
    await p.ready(12);
    await expect
        .poll(() => p.element("#source-view").textContent)
        .toContain("// 12");
    p.send({ type: "preview-source", revision: 13, source: "not valid Slint" });
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
        source: FIRST_BUTTON_SOURCE,
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
