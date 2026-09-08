// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { decodeValue, type SourceBytes, type SourceNode } from "./source";

export function maskComposition(
    children: readonly SourceNode<SourceBytes>[] = [],
): { kind: "rectangle"; maskId: string } | { kind: "raster" } | undefined {
    const visible = children.filter((n) => n.properties.visible !== false);
    const masks = visible.filter((n) => n.properties.isMask === true);
    if (masks.length === 0) return undefined;
    const mask = masks[0];
    const p = decodeValue(mask.properties);
    const solid =
        Array.isArray(p.fills) &&
        p.fills.length === 1 &&
        p.fills[0].type === "SOLID" &&
        p.fills[0].visible !== false &&
        p.fills[0].opacity === 1;
    if (
        masks.length === 1 &&
        visible[0] === mask &&
        mask.type === "RECTANGLE" &&
        p.opacity === 1 &&
        p.rotation === 0 &&
        [p.strokes, p.effects].every(
            (entries) =>
                entries === undefined ||
                (Array.isArray(entries) &&
                    entries.every((entry) => entry.visible === false)),
        ) &&
        [
            "cornerRadius",
            "topLeftRadius",
            "topRightRadius",
            "bottomLeftRadius",
            "bottomRightRadius",
        ].every((k) => p[k] === undefined || p[k] === 0) &&
        (p.maskType === "VECTOR" || (p.maskType === "ALPHA" && solid)) &&
        ["x", "y", "width", "height"].every(
            (k) => typeof p[k] === "number" && Number.isFinite(p[k]),
        ) &&
        Number(p.width) > 0 &&
        Number(p.height) > 0
    )
        return { kind: "rectangle", maskId: mask.id };
    return { kind: "raster" };
}
