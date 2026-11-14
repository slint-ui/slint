// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { SourceCapture, SourceNode } from "../src/plugin/source";
import { requireValue } from "../src/preview/slint-ir";

export function repeatedPreview(
    seed: SourceCapture,
    count: number,
): SourceCapture {
    const capture = structuredClone(seed);
    const original = requireValue(capture.root.children?.[0]);
    const library = requireValue(capture.components);
    const reference = library.references[original.id];
    const columns = 4;
    Object.assign(capture.root.properties, {
        width: 440,
        height: Math.ceil(count / columns) * 52,
        layoutMode: "NONE",
        fills: [],
    });
    capture.root.children = Array.from({ length: count }, (_, index) => {
        const node = structuredClone(original);
        const rename = (child: SourceNode) => {
            child.id += `:repeat-${index}`;
            if (child.type === "TEXT" || child.name === "Label") {
                child.type = "TEXT";
                Object.assign(child.properties, {
                    characters: `Label ${index}`,
                    fontName: { family: "Inter", style: "Regular" },
                    fontSize: 14,
                    fontWeight: 400,
                    textAutoResize: "NONE",
                    textAlignHorizontal: "LEFT",
                    textAlignVertical: "TOP",
                    lineHeight: { unit: "PIXELS", value: 20 },
                    letterSpacing: { unit: "PIXELS", value: 0 },
                });
            }
            child.children?.forEach(rename);
        };
        rename(node);
        Object.assign(node.properties, {
            x: (index % columns) * 110,
            y: Math.floor(index / columns) * 52,
            width: 95 + (index % 3),
            height: 40,
            fills: [
                {
                    type: "SOLID",
                    opacity: 1,
                    color: {
                        r: (index % 3) / 2,
                        g: ((index + 1) % 3) / 2,
                        b: 0.25,
                    },
                },
            ],
        });
        library.references[node.id] = reference;
        return node;
    });
    return capture;
}
