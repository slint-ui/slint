// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { SourceCapture, SourceNode } from "../src/plugin/source";
import { requireValue } from "../src/preview/slint-ir";

/** Small authored seed, expanded in memory; no Community capture is a fixture. */
export function buttonFamily(seed: SourceCapture): SourceCapture {
    const capture = structuredClone(seed);
    const library = requireValue(capture.components);
    const definition = library.definitions[0];
    const originals = definition.variants;
    definition.axes = {
        ...definition.axes,
        State: {
            defaultValue: "enabled",
            options: ["enabled", "hovered", "clicked", "disabled"],
        },
        Size: { defaultValue: "medium", options: ["small", "medium", "large"] },
        Align: { defaultValue: "start", options: ["start", "center"] },
        Style: { defaultValue: "solid", options: ["solid", "outline"] },
    };
    definition.variants = [];
    const contract = requireValue(definition.contract);
    const originalBindings = contract.bindings;
    contract.bindings = {};
    library.references = {};
    capture.root.children = [];
    Object.assign(capture.root.properties, {
        width: 1000,
        height: 1000,
        layoutWrap: "WRAP",
        itemSpacing: 8,
        counterAxisSpacing: 8,
    });
    for (const original of originals)
        for (const State of definition.axes.State.options)
            for (const Size of definition.axes.Size.options)
                for (const Align of definition.axes.Align.options)
                    for (const Style of definition.axes.Style.options) {
                        const index = definition.variants.length;
                        const root = structuredClone(original.root);
                        const suffix = `:sample-${index}`;
                        function visit(node: SourceNode) {
                            const binding = originalBindings[node.id];
                            node.id += suffix;
                            if (binding) contract.bindings[node.id] = binding;
                            if (node.name === "Label") {
                                node.type = "TEXT";
                                Object.assign(node.properties, {
                                    characters: "Button",
                                    fontName: {
                                        family: "Inter",
                                        style: "Regular",
                                    },
                                    fontSize: 14,
                                    fontWeight: 400,
                                    textAlignHorizontal: "LEFT",
                                    textAlignVertical: "TOP",
                                    lineHeight: { unit: "PIXELS", value: 20 },
                                    letterSpacing: { unit: "PIXELS", value: 0 },
                                    textAutoResize: "WIDTH_AND_HEIGHT",
                                    layoutSizingHorizontal: "HUG",
                                    layoutSizingVertical: "HUG",
                                });
                            }
                            node.children?.forEach(visit);
                        }
                        visit(root);
                        const padding =
                            Size === "small" ? 4 : Size === "medium" ? 6 : 10;
                        Object.assign(root.properties, {
                            paddingTop: padding,
                            paddingBottom: padding,
                            height: 20 + padding * 2,
                            primaryAxisAlignItems:
                                Align === "start" ? "MIN" : "CENTER",
                            opacity: State === "disabled" ? 0.5 : 1,
                            cornerRadius: State === "clicked" ? 2 : 4,
                        });
                        if (Style === "outline") root.properties.fills = [];
                        const values = {
                            ...original.values,
                            State,
                            Size,
                            Align,
                            Style,
                        };
                        definition.variants.push({ id: root.id, values, root });
                        const instance = structuredClone(root);
                        instance.type = "INSTANCE";
                        instance.id += ":instance";
                        library.references[instance.id] = {
                            definitionId: definition.id,
                            variantId: root.id,
                        };
                        capture.root.children?.push(instance);
                    }
    return capture;
}
