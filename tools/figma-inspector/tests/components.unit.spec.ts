// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";
import {
    planReachableVariants,
    type ReachabilityNode,
} from "../src/plugin/components";
import type { ComponentLibrary } from "../src/plugin/components";
import { validateComponentLibrary } from "../src/plugin/components";

describe("reachability", () => {
    const node = (
        id: string,
        children: ReachabilityNode[] = [],
    ): ReachabilityNode => ({ id, children });
    const library = (
        definitions: ComponentLibrary<ReachabilityNode>["definitions"],
        references: ComponentLibrary<ReachabilityNode>["references"] = {},
    ): ComponentLibrary<ReachabilityNode> => ({
        version: 1,
        definitions,
        references,
    });
    const family = (
        id: string,
        count: number,
        contract?: ComponentLibrary<ReachabilityNode>["definitions"][number]["contract"],
    ) => ({
        id,
        name: id,
        axes: {
            state: {
                defaultValue: "0",
                options: Array.from({ length: count }, (_, i) => String(i)),
            },
        },
        contract,
        variants: Array.from({ length: count }, (_, i) => ({
            id: `${id}-${i}`,
            values: { state: String(i) },
            root: node(`${id}-root-${i}`),
        })),
    });

    test("retains one or two private variants and follows nested closure", () => {
        const defs = [family("icons", 132), family("nested", 2)];
        const result = planReachableVariants(
            library(defs, {
                use: { definitionId: "icons", variantId: "icons-7" },
                nestedUse: { definitionId: "nested", variantId: "nested-1" },
            }),
            node("root", [node("use"), node("nestedUse")]),
            { kind: "frame" },
        );
        expect([...(result.variants.get("icons") ?? [])]).toEqual(["icons-7"]);
        expect([...(result.variants.get("nested") ?? [])]).toEqual([
            "nested-1",
        ]);
    });

    test("public component sets and components retain complete owners", () => {
        const defs = [family("public", 3), family("nested-public", 1)];
        defs[0].variants[2].root.children = [node("nested-public-use")];
        for (const kind of ["component-set", "component"] as const) {
            const result = planReachableVariants(
                library(defs, {
                    "nested-public-use": {
                        definitionId: "nested-public",
                        variantId: "nested-public-0",
                    },
                }),
                node("root"),
                {
                    kind,
                    definitionId: "public",
                },
            );
            expect(result.completeFamilies.has("public")).toBe(true);
            expect(result.variants.get("public")?.size).toBe(3);
            expect(
                result.variants.get("nested-public")?.has("nested-public-0"),
            ).toBe(true);
        }
    });

    test("cycles terminate and unresolved references fall back conservatively", () => {
        const defs = [family("a", 2), family("b", 2)];
        const result = planReachableVariants(
            library(defs, {
                a0: { definitionId: "a", variantId: "a-0" },
                b0: { definitionId: "b", variantId: "b-0" },
            }),
            node("a0", [node("b0", [node("a0")])]),
            { kind: "frame" },
        );
        expect(result.variants.get("a")?.size).toBe(1);
        expect(result.variants.get("b")?.size).toBe(1);
    });

    test("instance swap contracts retain the affected family and axes are deterministic", () => {
        const defs = [
            family("swap", 3, {
                version: 1,
                properties: {
                    target: { type: "INSTANCE_SWAP", defaultValue: "x" },
                },
                bindings: {},
            }),
            family("private", 2),
        ];
        const result = planReachableVariants(
            library(defs, {
                use: { definitionId: "swap", variantId: "swap-1" },
                private: { definitionId: "private", variantId: "private-1" },
            }),
            node("root", [node("use"), node("private")]),
            { kind: "frame" },
        );
        expect(result.completeFamilies.has("swap")).toBe(true);
        expect(result.axes.get("private")?.get("state")).toEqual({
            options: ["1"],
            defaultValue: "1",
        });
    });

    test("hidden boolean-bound nodes remain traversable", () => {
        const defs = [family("hidden", 2)];
        const result = planReachableVariants(
            library(defs, {
                hidden: { definitionId: "hidden", variantId: "hidden-1" },
            }),
            node("root", [node("hidden")]),
            { kind: "frame" },
        );
        expect(result.variants.get("hidden")?.has("hidden-1")).toBe(true);
    });

    test("unknown variant fallback traverses dependencies of every retained variant", () => {
        const defs = [family("fallback", 2), family("dependency", 1)];
        defs[0].variants[1].root.children = [node("dependency-use")];
        const result = planReachableVariants(
            library(defs, {
                fallbackUse: { definitionId: "fallback", variantId: "missing" },
                "dependency-use": {
                    definitionId: "dependency",
                    variantId: "dependency-0",
                },
            }),
            node("root", [node("fallbackUse")]),
            { kind: "frame" },
        );
        expect(result.completeFamilies.has("fallback")).toBe(true);
        expect(result.variants.get("dependency")?.has("dependency-0")).toBe(
            true,
        );
        expect(result.diagnostics.length).toBeGreaterThan(0);
    });

    test("component scope metadata is optional but validated", () => {
        const definition = family(
            "scoped",
            1,
        ) as ComponentLibrary<ReachabilityNode>["definitions"][number];
        definition.scope = "private";
        expect(() =>
            validateComponentLibrary(library([definition]), () => undefined),
        ).not.toThrow();
        expect(() =>
            validateComponentLibrary(
                library([{ ...definition, scope: "invalid" as never }]),
                () => undefined,
            ),
        ).toThrow();
    });
});
