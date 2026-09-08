// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";

import { readFile } from "node:fs/promises";
import { parseSnapshot, serializeSnapshot } from "../src/plugin/snapshot";
import { convertSnapshot, convertSnapshotJson } from "../src/preview/converter";
import {
    normalizeSvg,
    normalizeSvgToNodeBounds,
    utf8ToBase64,
} from "../src/images";
import { convertExport } from "../src/preview/convert-capture";

/** Repository fixture headers describe the test asset, not the user's design. */
async function readSlintFixture(path: string): Promise<string> {
    const source = await readFile(path, "utf8");
    return source.replace(
        /^\/\/ Copyright[^\n]*\n\/\/ SPDX-License-Identifier[:][^\n]*\n\n/,
        "",
    );
}

describe("converter", () => {
    test("round-trips and deterministically converts the canonical snapshot", async () => {
        const json = await readFile("fixtures/button.snapshot.json", "utf8");
        const parsed = parseSnapshot(json);
        expect(parsed.ok).toBe(true);
        if (!parsed.ok) return;
        expect(JSON.parse(serializeSnapshot(parsed.snapshot))).toEqual(
            JSON.parse(json),
        );

        const result = convertSnapshotJson(json);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toBe(
            await readSlintFixture("fixtures/button.slint"),
        );
        expect(convertSnapshotJson(json)).toEqual(result);
        expect(convertSnapshot(parsed.snapshot)).toEqual(result);

        const offsetSnapshot = {
            ...parsed.snapshot,
            root: { ...parsed.snapshot.root, x: 140, y: 85 },
        };
        const offsetResult = convertSnapshotJson(
            JSON.stringify(offsetSnapshot),
        );
        expect(offsetResult.ok).toBe(true);
        if (offsetResult.ok) {
            expect(offsetResult.source).toContain(
                "        x: 0px;\n        y: 0px;",
            );
        }
    });

    test("converts the canonical styled Frame with ordered appearance and children", async () => {
        const json = await readFile("fixtures/frame.snapshot.json", "utf8");
        const parsed = parseSnapshot(json);
        expect(parsed.ok).toBe(true);
        if (!parsed.ok) return;
        const result = convertSnapshotJson(json);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toBe(
            await readSlintFixture("fixtures/frame.slint"),
        );
        const framePosition = result.source.indexOf("    Rectangle {");
        const background = result.source.indexOf("        background:");
        const child = result.source.indexOf("        Text {");
        expect(framePosition).toBeGreaterThanOrEqual(0);
        expect(background).toBeGreaterThan(framePosition);
        expect(child).toBeGreaterThan(background);
        expect(result.source).toContain("        opacity: 0.92;");
        expect(result.source).toContain("            x: 16px;");
        expect(result.source).toContain("            opacity: 0.88;");
    });

    test("converts fixed horizontal auto-layout to FlexboxLayout with no-wrap", async () => {
        const json = await readFile(
            "fixtures/auto-layout.snapshot.json",
            "utf8",
        );
        const result = convertSnapshotJson(json);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toBe(
            await readSlintFixture("fixtures/auto-layout.slint"),
        );
        expect(result.source).toContain("FlexboxLayout {");
        expect(result.source).not.toContain("flex-direction: row;");
        expect(result.source).toContain("flex-wrap: no-wrap;");
        expect(result.source).not.toContain("HorizontalLayout");
        expect(result.source).not.toContain("VerticalLayout");
        expect(result.source.indexOf("alignment: center;")).toBeGreaterThan(
            result.source.indexOf("spacing: 12px;"),
        );
        expect(result.source).not.toMatch(/\n\s+x: [^0]/);
    });

    test("maps hug icon and label sizing to intrinsic Flexbox items", async () => {
        const json = await readFile(
            "fixtures/hug-button.snapshot.json",
            "utf8",
        );
        const result = convertSnapshotJson(json);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toBe(
            await readSlintFixture("fixtures/hug-button.slint"),
        );
        expect(result.source).not.toContain("min-width: 24px;");
        expect(result.source).not.toContain("preferred-width: 96px;");
        expect(result.source).not.toContain("min-height: 24px;");
        expect(result.source).not.toContain("preferred-height: 24px;");
        expect(result.source).not.toContain("max-width: 96px;");
        expect(result.source).toContain("horizontal-stretch: 0;");
    });

    test("maps fixed flex axes to dimensions while retaining hug and fill mappings", async () => {
        const json = await readFile(
            "fixtures/auto-layout.snapshot.json",
            "utf8",
        );
        const result = convertSnapshotJson(json);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toContain("width: 100px;");
        expect(result.source).toContain("height: 40px;");
        expect(result.source).not.toContain("max-width: 100px;");
        expect(result.source).not.toContain("max-height: 40px;");
    });

    test("uses text auto-resize modes to choose intrinsic geometry per axis", async () => {
        const base = JSON.parse(
            await readFile("fixtures/button.snapshot.json", "utf8"),
        ) as {
            root: {
                children: Array<Record<string, unknown>>;
            };
        };
        for (const [mode, expected] of [
            ["width-and-height", { width: false, height: false }],
            ["height", { width: true, height: false }],
            ["none", { width: true, height: true }],
            ["truncate", { width: true, height: true }],
        ] as const) {
            const snapshot = structuredClone(base);
            snapshot.root.children[1].textAutoResize = mode;
            const result = convertSnapshotJson(
                JSON.stringify({ ...base, root: snapshot.root }),
            );
            expect(result.ok, mode).toBe(true);
            if (!result.ok) continue;
            const textSource = result.source.slice(
                result.source.lastIndexOf("Text {"),
            );
            expect(
                textSource.includes("width: 240px;") || !expected.width,
                mode,
            ).toBe(true);
            expect(
                textSource.includes("height: 72px;") || !expected.height,
                mode,
            ).toBe(true);
        }
    });

    test("keeps fixed clipping and ending truncation explicit", async () => {
        const base = JSON.parse(
            await readFile("fixtures/button.snapshot.json", "utf8"),
        ) as {
            root: { children: Array<Record<string, unknown>> };
        };
        const fixed = structuredClone(base);
        fixed.root.children[1].textAutoResize = "none";
        fixed.root.children[1].overflow = "clip";
        const fixedResult = convertSnapshotJson(JSON.stringify(fixed));
        expect(fixedResult.ok).toBe(true);
        if (fixedResult.ok) {
            const textSource = fixedResult.source.slice(
                fixedResult.source.lastIndexOf("Text {"),
            );
            expect(textSource).toContain("width: 240px;");
            expect(textSource).toContain("height: 72px;");
            expect(textSource).not.toContain("overflow: elide;");
        }

        const truncated = structuredClone(base);
        truncated.root.children[1].textAutoResize = "truncate";
        truncated.root.children[1].overflow = "elide";
        const truncatedResult = convertSnapshotJson(JSON.stringify(truncated));
        expect(truncatedResult.ok).toBe(true);
        if (truncatedResult.ok) {
            const textSource = truncatedResult.source.slice(
                truncatedResult.source.lastIndexOf("Text {"),
            );
            expect(textSource).toContain("width: 240px;");
            expect(textSource).toContain("height: 72px;");
            expect(textSource).toContain("overflow: elide;");
        }
    });

    test("does not warn about unavailable fonts in supplied text snapshots", async () => {
        const snapshot = JSON.parse(
            await readFile("fixtures/button.snapshot.json", "utf8"),
        );
        snapshot.root.children[1].fontFamily = "Roboto";
        const result = convertSnapshot(snapshot);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(
            result.warnings.some(
                (warning) => warning.code === "FONT_FAMILY_FALLBACK",
            ),
        ).toBe(false);
    });

    test("wraps a selected intrinsic text root in a single-child layout", async () => {
        const base = JSON.parse(
            await readFile("fixtures/button.snapshot.json", "utf8"),
        ) as { root: Record<string, unknown> };
        const textRoot = {
            ...base.root,
            kind: "text",
            id: "root:text",
            name: "Button",
            characters: "Button",
            fills: [
                {
                    kind: "solid",
                    color: { r: 1, g: 1, b: 1, a: 1 },
                    opacity: 1,
                },
            ],
            fontFamily: "Inter",
            fontStyle: "Regular",
            fontSize: 16,
            fontWeight: 600,
            textAutoResize: "width-and-height",
            horizontalAlign: "CENTER",
            verticalAlign: "CENTER",
            italic: false,
            letterSpacing: 0,
            lineHeightFactor: null,
            wrap: false,
            overflow: "clip",
            maxLines: null,
            runs: [],
        };
        const result = convertSnapshotJson(
            JSON.stringify({ ...base, root: textRoot }),
        );
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toContain("FlexboxLayout {");
        expect(result.source).not.toContain("width: 240px;");
        expect(result.source).not.toContain("height: 72px;");
    });

    test("omits safe scalar defaults without dropping authored alignment", async () => {
        const defaults = convertSnapshotJson(
            await readFile("fixtures/default-heavy.snapshot.json", "utf8"),
        );
        expect(defaults.ok).toBe(true);
        if (!defaults.ok) return;
        expect(defaults.source).not.toContain("vertical-alignment: top;");
        expect(defaults.source).not.toContain("line-height-factor: 1;");
        expect(defaults.source).toContain("horizontal-alignment: left;");

        const invalidMaxLines = JSON.parse(
            await readFile("fixtures/default-heavy.snapshot.json", "utf8"),
        ) as { root: { children: Array<Record<string, unknown>> } };
        invalidMaxLines.root.children[1].maxLines = 0;
        const invalid = convertSnapshotJson(JSON.stringify(invalidMaxLines));
        expect(invalid.ok).toBe(false);
        if (!invalid.ok)
            expect(
                invalid.diagnostics.some((item) =>
                    item.propertyPath?.endsWith(".maxLines"),
                ),
            ).toBe(true);

        const svg = convertSnapshotJson(
            await readFile("fixtures/svg-multi-path.snapshot.json", "utf8"),
        );
        expect(svg.ok).toBe(true);
        if (svg.ok) expect(svg.source).toContain("image-fit: fill;");
    });

    test("collapses only provably transparent root and image-fill wrappers", async () => {
        const button = convertSnapshotJson(
            await readFile("fixtures/button.snapshot.json", "utf8"),
        );
        expect(button.ok).toBe(true);
        if (button.ok)
            expect((button.source.match(/Rectangle \{/g) ?? []).length).toBe(1);

        const image = convertSnapshotJson(
            await readFile("fixtures/image-fill.snapshot.json", "utf8"),
        );
        expect(image.ok).toBe(true);
        if (image.ok) expect(image.source).not.toContain("clip: true;");

        const root = JSON.parse(
            await readFile("fixtures/default-heavy.snapshot.json", "utf8"),
        ) as { root: Record<string, unknown> };
        const retained = convertSnapshotJson(
            JSON.stringify({ ...root, root: { ...root.root, opacity: 0.8 } }),
        );
        expect(retained.ok).toBe(true);
        if (retained.ok)
            expect(
                (retained.source.match(/Rectangle \{/g) ?? []).length,
            ).toBeGreaterThan(2);
    });

    test("retains Window transparency while omitting transparent Rectangle backgrounds", async () => {
        const base = JSON.parse(
            await readFile("fixtures/default-heavy.snapshot.json", "utf8"),
        ) as { root: Record<string, unknown> };
        const result = convertSnapshotJson(
            JSON.stringify({
                ...base,
                root: {
                    ...base.root,
                    kind: "frame",
                    fills: [],
                    strokes: [],
                    cornerRadii: [0, 0, 0, 0],
                    clipsContent: false,
                    autoLayout: null,
                    shadows: [
                        {
                            color: { r: 0, g: 0, b: 0, a: 0.5 },
                            offsetX: 0,
                            offsetY: 0,
                            blur: 4,
                            spread: 0,
                        },
                        {
                            color: { r: 0, g: 0, b: 0, a: 0.25 },
                            offsetX: 0,
                            offsetY: 0,
                            blur: 0,
                            spread: 0,
                        },
                    ],
                },
            }),
        );
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source.match(/background: transparent;/g)?.length).toBe(
            1,
        );
        expect(result.source).toContain("drop-shadow-color: #00000040;");

        const spreadResult = convertSnapshotJson(
            JSON.stringify({
                ...base,
                root: {
                    ...base.root,
                    kind: "frame",
                    containerKind: "native",
                    fills: [],
                    strokes: [],
                    cornerRadii: [12, 12, 12, 12],
                    clipsContent: false,
                    autoLayout: null,
                    shadows: [
                        {
                            color: { r: 0, g: 0, b: 0, a: 0.1 },
                            offsetX: 0,
                            offsetY: 8,
                            blur: 24,
                            spread: -6,
                        },
                    ],
                },
            }),
        );
        expect(spreadResult.ok).toBe(true);
        if (!spreadResult.ok) return;
        expect(spreadResult.source).toContain("drop-shadow-spread: -6px;");
        expect(spreadResult.warnings).toEqual(
            expect.arrayContaining([
                expect.objectContaining({
                    code: "FEMTOVG_SHADOW_SPREAD_UNSUPPORTED",
                    originalValue: "-6px",
                    category: "approximation",
                }),
            ]),
        );

        const rectangleRoot = {
            ...base.root,
            kind: "rectangle",
            fills: [],
            strokes: [],
            shadows: [],
            cornerRadii: [0, 0, 0, 0],
            clipsContent: false,
        } as Record<string, unknown>;
        rectangleRoot.children = undefined;
        const rectangle = convertSnapshotJson(
            JSON.stringify({ ...base, root: rectangleRoot }),
        );
        expect(rectangle.ok).toBe(true);
        if (rectangle.ok)
            expect(
                rectangle.source.match(/background: transparent;/g)?.length,
            ).toBe(1);
    });

    test("maps nested fill items and overrides primary alignment with stretch", async () => {
        const json = await readFile(
            "fixtures/nested-fill.snapshot.json",
            "utf8",
        );
        const result = convertSnapshotJson(json);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toBe(
            await readSlintFixture("fixtures/nested-fill.slint"),
        );
        expect(result.source.match(/FlexboxLayout \{/g)?.length).toBe(2);
        expect(result.source).toContain("flex-direction: column;");
        expect(result.source).not.toContain("flex-direction: row;");
        expect(result.source).toContain("alignment: stretch;");
        expect(result.source).toContain("min-width: 0px;");
        expect(result.source).not.toMatch(
            /Rectangle \{\n\s+width: 44px;\n\s+preferred-height: 44px;\n\s+min-height: 0px;/u,
        );
        expect(result.source).toContain("horizontal-stretch: 1;");
        expect(result.source).toContain("cross-axis-self-alignment: stretch;");
        expect(result.source.indexOf('text: "Nested settings"')).toBeLessThan(
            result.source.indexOf('text: "Every setting is live"'),
        );
    });

    test("uses resolved dimensions for hug containers with fill descendants", async () => {
        const snapshot = JSON.parse(
            await readFile("fixtures/nested-instance.snapshot.json", "utf8"),
        ) as {
            root: { children: Array<Record<string, unknown>> };
        };
        snapshot.root.children[0].layoutSizingHorizontal = "hug";
        snapshot.root.children[0].layoutSizingVertical = "hug";

        const result = convertSnapshot(snapshot);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toContain("width: 144px;");
        expect(result.source).toContain("height: 48px;");
        expect(result.source).not.toContain("preferred-width: 144px;");
        expect(result.source).not.toContain("preferred-height: 48px;");
    });

    test("converts resolved Component and Instance containers through the v7 boundary", async () => {
        const component = await readFile(
            "fixtures/component-text.snapshot.json",
            "utf8",
        );
        const componentResult = convertSnapshotJson(component);
        expect(componentResult.ok).toBe(true);
        if (!componentResult.ok) return;
        expect(componentResult.source).toBe(
            await readSlintFixture("fixtures/component-text.slint"),
        );

        const instance = convertSnapshotJson(
            await readFile("fixtures/instance-override.snapshot.json", "utf8"),
        );
        expect(instance.ok).toBe(true);
        if (!instance.ok) return;
        expect(instance.source).toContain('text: "Continue now";');
        expect(instance.source).toContain("FlexboxLayout {");

        const nested = convertSnapshotJson(
            await readFile("fixtures/nested-instance.snapshot.json", "utf8"),
        );
        expect(nested.ok).toBe(true);
        if (!nested.ok) return;
        expect(nested.source.match(/FlexboxLayout \{/g)?.length).toBe(2);
        expect(nested.source).toContain('text: "Nested override";');
    });

    test("converts the canonical Instance button to its committed Slint golden", async () => {
        const json = await readFile(
            "fixtures/instance-button.snapshot.json",
            "utf8",
        );
        const result = convertSnapshotJson(json);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toBe(
            await readSlintFixture("fixtures/instance-button.slint"),
        );
        expect(result.source).toContain("Image {");
        expect(result.source).toContain("image-fit: fill;");
        expect(result.source).toContain('text: "Continue now";');
        expect(convertSnapshotJson(json)).toEqual(result);
    });

    test("keeps auto-layout edits deterministic and local to their mapped properties", async () => {
        const snapshot = JSON.parse(
            await readFile("fixtures/hug-button.snapshot.json", "utf8"),
        ) as Record<string, unknown> & {
            root: Record<string, unknown> & {
                autoLayout: Record<string, unknown>;
                children: Array<Record<string, unknown>>;
            };
        };
        const baseline = convertSnapshotJson(JSON.stringify(snapshot));
        const changed = {
            ...snapshot,
            root: {
                ...snapshot.root,
                autoLayout: {
                    ...snapshot.root.autoLayout,
                    paddingLeft: 24,
                    itemSpacing: 18,
                    primaryAlignment: "end",
                },
                children: [
                    { ...snapshot.root.children[0] },
                    {
                        ...snapshot.root.children[1],
                        characters: "Continue now",
                    },
                ],
            },
        };
        const updated = convertSnapshotJson(JSON.stringify(changed));
        expect(baseline.ok).toBe(true);
        expect(updated.ok).toBe(true);
        if (!baseline.ok || !updated.ok) return;
        expect(updated.source).toContain("padding-left: 24px;");
        expect(updated.source).toContain("spacing: 18px;");
        expect(updated.source).toContain("alignment: end;");
        expect(updated.source).toContain('text: "Continue now";');
        expect(updated.source).not.toContain('text: "Continue";');
    });

    test("reports malformed snapshot JSON and unsupported schema values", async () => {
        expect(convertSnapshotJson("not json")).toMatchObject({
            ok: false,
            diagnostics: [{ code: "INVALID_SNAPSHOT_JSON" }],
        });
        const invalid = convertSnapshotJson(
            JSON.stringify({ schemaVersion: 99 }),
        );
        expect(invalid.ok).toBe(false);
        if (!invalid.ok)
            expect(invalid.diagnostics[0].code).toBe("INVALID_SNAPSHOT");
    });

    test("normalizes and base64-encodes SVG as deterministic UTF-8", () => {
        expect(normalizeSvg("  <svg>\r\n☃\r</svg>  ")).toBe("<svg>\n☃\n</svg>");
        expect(utf8ToBase64("☃ 世界")).toBe("4piDIOS4lueVjA==");
    });

    test("restores node bounds without scaling SVG child paths", () => {
        const svg =
            '<svg width="23" height="17" viewBox="0 0 23 17"><path d="M1 2h20v3H1z" /></svg>';
        expect(normalizeSvgToNodeBounds(svg, 24, 24)).toBe(
            '<svg width="24" height="24" viewBox="-0.5 -3.5 24 24"><path d="M1 2h20v3H1z" /></svg>',
        );
        expect(normalizeSvgToNodeBounds("<svg />", 24, 24)).toBe(
            '<svg width="24" height="24" viewBox="0 0 24 24"/>',
        );
    });

    test("converts complex SVG snapshot goldens to inline Slint Images", async () => {
        for (const fixture of [
            "svg-multi-path",
            "svg-mask-gradient",
            "svg-unicode",
        ]) {
            const json = await readFile(
                `fixtures/${fixture}.snapshot.json`,
                "utf8",
            );
            const result = convertSnapshotJson(json);
            expect(result.ok, fixture).toBe(true);
            if (!result.ok) continue;
            expect(result.source, fixture).toBe(
                await readSlintFixture(`fixtures/${fixture}.slint`),
            );
            expect(convertSnapshotJson(json), fixture).toEqual(result);
        }
    });

    test("keeps font-backed SVG conversion fill-only and free of synthetic strokes", async () => {
        const json = await readFile("tests/font-icon.snapshot.json", "utf8");
        const snapshot = JSON.parse(json) as {
            root: {
                paintBounds?: {
                    x: number;
                    y: number;
                    width: number;
                    height: number;
                };
                svg: string;
            };
        };
        const result = convertSnapshotJson(json);
        expect(result.ok).toBe(true);
        if (!result.ok) return;

        expect(result.source).toContain(
            `base64,${utf8ToBase64(normalizeSvg(snapshot.root.svg))}`,
        );
        expect(result.source).toContain("image-fit: fill;");
        expect(result.source).not.toContain("border-width:");
        expect(result.source).not.toContain("stroke-width:");

        snapshot.root.paintBounds = { x: 0, y: 0, width: 24, height: 24 };
        const paintedBoundsResult = convertSnapshotJson(
            JSON.stringify(snapshot),
        );
        expect(paintedBoundsResult.ok).toBe(true);
        if (!paintedBoundsResult.ok) return;
        expect(paintedBoundsResult.source).not.toContain(
            "image-rendering: smooth;",
        );
    });

    test("uses Figma's raster sidecar for captured SVG nodes", async () => {
        const json = await readFile("tests/font-icon.snapshot.json", "utf8");
        const snapshot = JSON.parse(json) as {
            root: {
                opacity: number;
                raster?: {
                    data: string;
                    exportScale: number;
                    pixelWidth: number;
                    pixelHeight: number;
                };
                svg: string;
            };
        };
        snapshot.root.raster = {
            data: await readFile("fixtures/authored/square.png", "base64"),
            exportScale: 1,
            pixelWidth: 24,
            pixelHeight: 24,
        };
        snapshot.root.opacity = 0.5;
        const result = convertSnapshotJson(JSON.stringify(snapshot));
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toContain(
            `source: @image-url("data:image/png;base64,${snapshot.root.raster.data}");`,
        );
        expect(result.source).toContain("image-fit: fill;");
        expect(result.source).not.toContain("opacity: 0.5;");
        expect(result.source).not.toContain("data:image/svg+xml");
    });

    test("validates raster PNG headers, dimensions, and density metadata", async () => {
        const json = JSON.parse(
            await readFile("tests/font-icon.snapshot.json", "utf8"),
        ) as { root: Record<string, unknown> };
        const data = await readFile(
            "fixtures/authored/asymmetric.png",
            "base64",
        );
        const valid = {
            ...json,
            root: {
                ...json.root,
                raster: {
                    data,
                    exportScale: 1,
                    pixelWidth: 32,
                    pixelHeight: 24,
                },
            },
        };
        expect(parseSnapshot(JSON.stringify(valid)).ok).toBe(true);
        for (const [name, raster] of [
            ["signature-only", { ...valid.root.raster, data: "iVBORw0KGgo=" }],
            ["wrong-width", { ...valid.root.raster, pixelWidth: 24 }],
            ["wrong-density", { ...valid.root.raster, exportScale: 0 }],
        ] as const) {
            const result = parseSnapshot(
                JSON.stringify({ ...valid, root: { ...valid.root, raster } }),
            );
            expect(result.ok, name).toBe(false);
        }
    });

    test("rejects malformed SVG snapshot content with stable diagnostics", async () => {
        const json = JSON.parse(
            await readFile("fixtures/svg-multi-path.snapshot.json", "utf8"),
        ) as { root: Record<string, unknown> };
        for (const [key, value] of [
            ["sourceType", "RECTANGLE"],
            ["svg", "<path />"],
        ] as const) {
            const malformed = JSON.stringify({
                ...json,
                root: { ...json.root, [key]: value },
            });
            expect(convertSnapshotJson(malformed), key).toMatchObject({
                ok: false,
                diagnostics: [
                    expect.objectContaining({ code: "INVALID_SNAPSHOT" }),
                ],
            });
            expect(convertSnapshot(JSON.parse(malformed)), key).toEqual(
                convertSnapshotJson(malformed),
            );
        }
    });

    test("warns when absolute and flow children require z-order approximation", async () => {
        const base = JSON.parse(
            await readFile("fixtures/auto-layout.snapshot.json", "utf8"),
        ) as {
            root: Record<string, unknown>;
            selection: unknown;
            schemaVersion: number;
        };
        const children = base.root.children as Record<string, unknown>[];
        const absolute = {
            ...children[0],
            id: "absolute",
            layoutPositioning: "absolute",
        };
        const flow = { ...children[0], id: "flow", layoutPositioning: "auto" };
        const result = convertSnapshot({
            ...base,
            root: { ...base.root, children: [absolute, flow] },
        });
        expect(result).toMatchObject({
            ok: true,
            warnings: [
                expect.objectContaining({
                    code: "ABSOLUTE_Z_ORDER_APPROXIMATED",
                    severity: "warning",
                }),
            ],
        });
    });

    test("rejects malformed embedded image payloads at the snapshot boundary", async () => {
        const base = JSON.parse(
            await readFile("fixtures/auto-layout.snapshot.json", "utf8"),
        ) as {
            root: Record<string, unknown>;
            selection: unknown;
            schemaVersion: number;
        };
        const imageRoot = {
            ...base.root,
            kind: "rectangle",
            fills: [
                {
                    kind: "image",
                    mimeType: "image/png",
                    data: "not base64!",
                    intrinsicWidth: 1,
                    intrinsicHeight: 1,
                    scaleMode: "FIT",
                    opacity: 1,
                },
            ],
            strokes: [],
            cornerRadii: [0, 0, 0, 0],
            clipsContent: false,
            shadows: [],
        };
        const malformedImage = convertSnapshot({ ...base, root: imageRoot });
        expect(malformedImage).toMatchObject({
            ok: false,
            diagnostics: [
                expect.objectContaining({ propertyPath: "root.fills.0.data" }),
            ],
        });
    });

    test("maps finite root and nested rotations to center-origin Slint transforms", async () => {
        const base = JSON.parse(
            await readFile("fixtures/auto-layout.snapshot.json", "utf8"),
        ) as {
            root: Record<string, unknown>;
            selection: unknown;
            schemaVersion: number;
        };
        const rotated = convertSnapshot({
            ...base,
            root: {
                ...base.root,
                rotation: 17,
                children: [
                    {
                        ...(base.root.children as Record<string, unknown>[])[0],
                        rotation: -8,
                    },
                ],
            },
        });
        expect(rotated.ok).toBe(true);
        if (!rotated.ok) return;
        expect(rotated.source).toContain("transform-rotation: 17deg;");
        expect(rotated.source).toContain("transform-rotation: -8deg;");
        expect(rotated.source).toContain(
            "transform-origin: { x: self.width / 2, y: self.height / 2 };",
        );
    });

    test("converts non-Inter text image JSON deterministically without font fallback", async () => {
        const json = await readFile(
            "tests/non-inter-text.snapshot.json",
            "utf8",
        );
        const result = convertSnapshotJson(json);
        expect(result.ok).toBe(true);
        if (!result.ok) return;
        expect(result.source).toContain("Image {");
        expect(result.source).toContain("data:image/svg+xml;base64,");
        expect(result.source).not.toContain("Text {");
        expect(result.warnings).not.toEqual(
            expect.arrayContaining([
                expect.objectContaining({ code: "FONT_FAMILY_FALLBACK" }),
            ]),
        );
        expect(convertSnapshotJson(json)).toEqual(result);
    });
});

describe("release-regressions", () => {
    test("CRLF text emits only supported Slint escapes", async () => {
        const result = convertSnapshotJson(
            await readFile("fixtures/regressions/crlf.snapshot.json", "utf8"),
        );
        expect(result.ok).toBe(true);
        if (!result.ok) throw Error("Conversion failed");
        expect(result.source).toContain("First\\u{d}\\nSecond");
    });

    test("native styled text preserves paint opacity", async () => {
        const result = await convertExport({
            type: "preview-capture",
            revision: 1,
            captureJson: await readFile(
                "fixtures/source/translucent-styled-text.json",
                "utf8",
            ),
        });
        expect(result.source).toContain('color=\\"#FFFFFF33\\"');
        expect(result.source).not.toContain('color=\\"#FFFFFF\\"');
    });
});
