// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";
import {
    predicate,
    selectValues,
    type VariantAxis,
} from "../src/preview/component-variants";
import { appearanceStates } from "../src/preview/component-variants";
import {
    variantDecision,
    type DecisionAxis,
} from "../src/preview/component-variants";

describe("predicates", () => {
    const axes: VariantAxis[] = [
        {
            key: "Color",
            property: "color",
            type: "Color",
            options: new Map([
                ["Primary", "primary"],
                ["Danger", "danger"],
            ]),
        },
        {
            key: "State",
            property: "state",
            type: "State",
            options: new Map([
                ["Enabled", "enabled"],
                ["Pressed", "pressed"],
                ["Disabled", "disabled"],
            ]),
        },
    ];
    const values = [
        { Color: "Primary", State: "Enabled" },
        { Color: "Primary", State: "Pressed" },
        { Color: "Primary", State: "Disabled" },
        { Color: "Danger", State: "Enabled" },
        { Color: "Danger", State: "Pressed" },
    ];
    function evaluate(code: string, color: string, state: string) {
        return new Function("root", "Color", "State", `return (${code})`)(
            { color, state },
            { primary: "Primary", danger: "Danger" },
            { enabled: "Enabled", pressed: "Pressed", disabled: "Disabled" },
        );
    }
    test("predicate reduction exactly preserves the sparse finite domain", () => {
        const code = predicate(values, axes);
        for (const Color of ["Primary", "Danger"])
            for (const State of ["Enabled", "Pressed", "Disabled"]) {
                expect(evaluate(code, Color, State)).toBe(
                    values.some((v) => v.Color === Color && v.State === State),
                );
            }
    });
    test("appearance selection eliminates an irrelevant axis without changing supported values", () => {
        const cases = values.map((v) => ({
            values: v,
            code: v.State === "Pressed" ? "2" : "1",
        }));
        const code = selectValues(cases, axes, "1");
        expect(code).not.toContain("Color");
        for (const c of cases)
            expect(evaluate(code, c.values.Color, c.values.State)).toBe(
                Number(c.code),
            );
    });
});

describe("states", () => {
    test("appearance partitions are exclusive and reset unspecified properties to the authored base", () => {
        const axes: VariantAxis[] = ["state", "style"].map((key) => ({
            key,
            property: key,
            type: "int",
            options: new Map([
                ["0", "0"],
                ["1", "1"],
            ]),
        }));
        const samples = [
            {
                values: { state: "0", style: "0" },
                properties: { background: "blue", "border-width": "0px" },
            },
            {
                values: { state: "1", style: "0" },
                properties: { background: "gray", "border-width": "0px" },
            },
            {
                values: { state: "0", style: "1" },
                properties: { background: "blue", "border-width": "1px" },
            },
            {
                values: { state: "1", style: "1" },
                properties: { background: "gray", "border-width": "1px" },
            },
        ];
        const source = appearanceStates(samples, axes, 1).join("\n");
        const states = [
            ...source.matchAll(/([\w-]+) when ([\s\S]*?): \{([^}]+)\}/g),
        ];
        for (const sample of samples) {
            const active = states.filter(([, , condition]) =>
                Function(
                    "root",
                    `return ${condition}`,
                )({
                    state: Number(sample.values.state),
                    style: Number(sample.values.style),
                }),
            );
            expect(active.length).toBeLessThanOrEqual(1);
            const properties = { ...samples[0].properties };
            for (const [, field, value] of (active[0]?.[3] ?? "").matchAll(
                /([\w-]+): ([^;]+);/g,
            ))
                Object.assign(properties, { [field]: value });
            expect(properties).toEqual(sample.properties);
        }
        expect(source).not.toContain("option-");
    });
});

describe("decision", () => {
    const axes: DecisionAxis[] = ["color", "style", "state"].map((key) => ({
        key,
        options: ["0", "1", "2"],
        finite: true,
        test: (value) => `root.${key} == ${value}`,
    }));
    const tuples = axes[0].options.flatMap((color) =>
        axes[1].options.flatMap((style) =>
            axes[2].options.map((state) => ({ color, style, state })),
        ),
    );

    test("factored decisions preserve every tuple across sparse finite domains", () => {
        // Deterministic varied truth tables, including empty/full domains and holes.
        for (let seed = 0; seed < 64; seed++) {
            const rows = tuples.filter(
                (_, i) => (i * 17 + seed * 13) % 31 < seed % 32,
            );
            const expression = variantDecision(rows, axes);
            const evaluate = Function("root", `return ${expression}`);
            for (const tuple of tuples)
                expect(evaluate(tuple)).toBe(rows.includes(tuple));
            expect(variantDecision([...rows].reverse(), axes)).toBe(expression);
        }
    });

    test("numeric axes retain a finite guard even when every captured value exists", () => {
        const axis = { ...axes[0], finite: false };
        const expression = variantDecision(
            axis.options.map((color) => ({ color })),
            [axis],
        );
        const evaluate = Function("root", `return ${expression}`);
        for (const color of [-1, 0, 1, 2, 3, 999])
            expect(evaluate({ color })).toBe(color >= 0 && color <= 2);
    });

    test("shared suffixes and complements avoid enumerating common states", () => {
        const rows = tuples.filter(
            (r) => r.color === "0" || (r.color === "1" && r.state !== "2"),
        );
        const expression = variantDecision(rows, axes);
        expect(expression).not.toContain("style");
        expect(expression.length).toBeLessThan(100);
    });

    test("boolean and integer domains keep their authored meaning", async () => {
        const { axisScalar } = await import(
            "../src/preview/component-variants"
        );
        expect(axisScalar(["False", "True"])).toBe("bool");
        expect(axisScalar(["1", "2", "3"])).toBe("int");
        expect(axisScalar(["01", "02"])).toBeUndefined();
        expect(axisScalar(["enabled", "disabled"])).toBeUndefined();
        expect(axisScalar(["true", "false", "mixed"])).toBeUndefined();
    });

    test("paint enums can vary while layout configuration remains structural", async () => {
        const { binding } = await import("../src/preview/slint-ir");
        expect(binding("alignment", "center").value.kind).toBe("raw");
        expect(binding("image-fit", "contain").value.kind).toBe("literal");
        expect(binding("alignment", "root.custom-alignment").value.kind).toBe(
            "raw",
        );
        expect(binding("unknown-property", "center").value.kind).toBe("raw");
    });
});
