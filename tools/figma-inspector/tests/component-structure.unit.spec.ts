// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";
import {
    elementShape,
    structuralSignature,
} from "../src/preview/component-structure";
import { literal } from "../src/preview/slint-ir";
import type { Element } from "../src/preview/slint-ir";
import { childOrder } from "../src/preview/component-structure";

describe("signatures", () => {
    const deepTree = (depth: number): Element =>
        depth === 0
            ? { type: "Rectangle", bindings: [], children: [] }
            : {
                  type: "Rectangle",
                  bindings: [],
                  children: [deepTree(depth - 1)],
              };

    const element = (
        type: string,
        code: string,
        children: Element[] = [],
    ): Element => ({
        type,
        bindings: [
            {
                kind: "binding",
                name: "value",
                value: literal("string", code),
                depth: 0,
            },
        ],
        children,
    });

    test("partial sibling orders can insert a missing decoration without cloning trees", () => {
        expect(
            childOrder([["label"], ["icon", "label"], ["label", "spinner"]]),
        ).toEqual(["icon", "label", "spinner"]);
        expect(
            childOrder([
                ["a", "b"],
                ["b", "a"],
            ]),
        ).toBeUndefined();
        expect(childOrder([["a", "a"]])).toBeUndefined();
    });
    const rawElement = (code: string): Element => ({
        type: "Rectangle",
        bindings: [
            {
                kind: "binding",
                name: "value",
                value: { kind: "raw", type: "string", code },
                depth: 0,
            },
        ],
        children: [],
    });

    describe("component structural signatures", () => {
        test("preserve type, condition, binding value, and child order distinctions", () => {
            const base = element("Rectangle", '"left"', [
                element("Text", '"child"'),
                element("Image", '"icon"'),
            ]);
            const type = element("Path", '"left"', [
                element("Text", '"child"'),
                element("Image", '"icon"'),
            ]);
            const condition = element("Rectangle", '"left"', [
                element("Text", '"child"'),
                element("Image", '"icon"'),
            ]);
            condition.children[0].condition = "root.visible";
            const binding = element("Rectangle", '"right"', [
                element("Text", '"child"'),
                element("Image", '"icon"'),
            ]);
            const order = element("Rectangle", '"left"', [
                element("Image", '"icon"'),
                element("Text", '"child"'),
            ]);
            expect(
                new Set(
                    [type, condition, binding, order].map(structuralSignature),
                ).size,
            ).toBe(4);
            for (const changed of [type, condition, binding, order])
                expect(structuralSignature(base)).not.toBe(
                    structuralSignature(changed),
                );
        });

        test("shape captures the same structural distinctions", () => {
            const base = element("Rectangle", '"left"', [
                element("Text", '"child"'),
            ]);
            const changedType = element("Path", '"left"', [
                element("Text", '"child"'),
            ]);
            const changedCondition = element("Rectangle", '"left"', [
                element("Text", '"child"'),
            ]);
            changedCondition.children[0].condition = "root.visible";
            const changedValue = rawElement("root.value");
            expect(
                new Set(
                    [base, changedType, changedCondition, changedValue].map(
                        elementShape,
                    ),
                ).size,
            ).toBe(4);
        });

        test("serialize deep trees once without recursive escaping", () => {
            const shallow = structuralSignature(deepTree(8));
            const deep = structuralSignature(deepTree(80));
            expect(deep.length).toBeGreaterThan(shallow.length * 8);
            expect(deep.length).toBeLessThan(shallow.length * 15);
            expect(deep).not.toContain('\\"{\\"type\\"');
        });
    });
});
