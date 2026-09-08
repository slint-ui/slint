// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { readFile } from "node:fs/promises";
import { expect, test } from "vitest";
import { buttonFamily } from "./button-family";
import { normalizeSource } from "../src/plugin/normalize";
import { convertSnapshot } from "../src/preview/converter";
import { childRoles } from "../src/preview/component-structure";
import { binding, type Element } from "../src/preview/slint-ir";

async function family() {
    const capture = buttonFamily(
        JSON.parse(
            await readFile("fixtures/source/conditional-root.json", "utf8"),
        ),
    );
    const normalized = await normalizeSource(capture, "export");
    if (!normalized.ok || normalized.empty) throw Error("Invalid family");
    return normalized.snapshot;
}

test("alignment and size bindings remain independent of state", async () => {
    const snapshot = await family();
    const generated = convertSnapshot(snapshot, { target: "export" });
    if (!generated.ok) throw Error(JSON.stringify(generated));
    const source = generated.source;
    for (const match of source.matchAll(
        /(?:alignment|padding-top|padding-bottom): ([\s\S]*?);/g,
    )) {
        expect(match[1]).not.toContain("variant-state");
        expect(match[1]).not.toContain("State.");
    }
    expect(source).not.toContain(" != ");
    for (const match of source.matchAll(/if ([^\n]+):/g))
        expect(match[1]).not.toContain(" == ");
    expect(source).toContain("private property <bool>");
    expect(convertSnapshot(snapshot, { target: "export" })).toEqual(generated);
});

test("preview specializes known occurrences without changing the reusable public contract", async () => {
    const snapshot = await family();
    const before = JSON.stringify(snapshot);
    const reusable = convertSnapshot(snapshot);
    const preview = convertSnapshot(snapshot, { specialize: true });
    if (!preview.ok || !reusable.ok) throw Error("Invalid conversion");
    expect(preview.source).not.toMatch(/\bif\b|variant-state|states \[/);
    expect(reusable.source).toContain(
        "in property <RootSizingState> variant-state",
    );
    expect(reusable.source).toContain("in property <string> label");
    expect(JSON.stringify(snapshot)).toBe(before);
});

test("same-type elements with distinct authored roles are not merged", () => {
    const node = (name: string): Element => ({
        type: "Rectangle",
        origin: { id: name, name },
        bindings: [binding("width", "10px")],
        children: [],
    });
    const root = (name: string): Element => ({
        type: "Rectangle",
        bindings: [],
        children: [node(name)],
    });
    expect(childRoles([root("Leading"), root("Trailing")])).toEqual([
        ["Rectangle:Leading"],
        ["Rectangle:Trailing"],
    ]);
});

test("opposite content conditions reuse one named boolean", async () => {
    const capture = JSON.parse(
        await readFile("fixtures/source/conditional-root.json", "utf8"),
    );
    const definition = capture.components.definitions[0];
    definition.variants = definition.variants.filter(
        (variant: { values: { Position: string } }) =>
            variant.values.Position !== "both",
    );
    definition.axes.Position.options = ["leading", "trailing"];
    const normalized = await normalizeSource(capture);
    if (!normalized.ok || normalized.empty) throw Error("Invalid fixture");
    const generated = convertSnapshot(normalized.snapshot);
    if (!generated.ok) throw Error(JSON.stringify(generated));
    expect(generated.source).toContain("private property <bool> has-leading:");
    expect(generated.source).toContain("if root.has-leading && root.icons:");
    expect(generated.source).toContain("if !root.has-leading && root.icons:");
});

test("true structural alternatives keep an unconditional layout and pass only required properties", async () => {
    const capture = JSON.parse(
        await readFile("fixtures/source/conditional-root.json", "utf8"),
    );
    // The same two roles swap order; a layout binding cannot express this change.
    capture.components.definitions[0].variants[0].root.children.reverse();
    const normalized = await normalizeSource(capture);
    if (!normalized.ok || normalized.empty) throw Error("Invalid fixture");
    const generated = convertSnapshot(normalized.snapshot);
    if (!generated.ok) throw Error(JSON.stringify(generated));
    expect(generated.source).toMatch(/FlexboxLayout \{\n\s+padding: 0px;/);
    const privateParts = generated.source.split(
        "export component RootSizing",
    )[0];
    expect(privateParts).toMatch(/component RootSizing\w+ inherits Rectangle/);
    expect(privateParts).not.toContain("variant-position:");
    expect(privateParts).not.toContain("in property <string> label");
    expect(privateParts).toContain("in property <bool> icons");
});
