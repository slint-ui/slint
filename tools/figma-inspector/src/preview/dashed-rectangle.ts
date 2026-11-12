// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type {
    SnapshotRectangleNode,
    SnapshotFrameLikeNode,
    SnapshotStroke,
} from "../plugin/snapshot";
import { binding, openElement, closeElement, type SlintLine } from "./slint-ir";
import { number, paintInBounds } from "./converter-context";

export function dashedRectangleSource(
    node: SnapshotRectangleNode | SnapshotFrameLikeNode,
    stroke: SnapshotStroke,
    depth: number,
): SlintLine[] {
    const weight = stroke.strokeTopWeight;
    if (weight === 0) return [];
    const expansion =
        stroke.align === "outside"
            ? weight
            : stroke.align === "center"
              ? weight / 2
              : 0;
    const width = node.width + expansion * 2;
    const height = node.height + expansion * 2;
    const brush = paintInBounds(stroke.paint, node, {
        x: -expansion,
        y: -expansion,
        width,
        height,
    });
    const delta = weight / 2 - expansion;
    const radii = node.cornerRadii.map((radius) =>
        Math.min(radius, node.width / 2, node.height / 2),
    );
    const move = (x: number, y: number) =>
        `${number(x + expansion)} ${number(y + expansion)}`;
    const pattern =
        stroke.dashPattern.length % 2
            ? [...stroke.dashPattern, ...stroke.dashPattern]
            : stroke.dashPattern;
    const period = pattern.reduce((sum, entry) => sum + entry, 0);
    const sourceDepth = depth;
    const source = (
        commands: string,
        dashes: readonly number[],
        offset = 0,
        depth = sourceDepth,
        origin = { x: -expansion, y: -expansion },
    ): SlintLine[] => [
        openElement("Path", depth),
        binding("x", `${number(origin.x)}px`, depth + 1),
        binding("y", `${number(origin.y)}px`, depth + 1),
        binding("width", `${number(width)}px`, depth + 1),
        binding("height", `${number(height)}px`, depth + 1),
        // Slint reserves half the stroke width around the path's view box.
        binding("viewbox-x", weight / 2, depth + 1),
        binding("viewbox-y", weight / 2, depth + 1),
        binding("viewbox-width", Math.max(0.001, width - weight), depth + 1),
        binding("viewbox-height", Math.max(0.001, height - weight), depth + 1),
        binding("commands", `"${commands}"`, depth + 1),
        binding("stroke", brush, depth + 1),
        binding("stroke-width", `${number(weight)}px`, depth + 1),
        ...(dashes.length
            ? [
                  binding(
                      "stroke-dash-array",
                      `[${dashes.map((length) => `${number(length)}px`).join(", ")}]`,
                      depth + 1,
                  ),
                  binding(
                      "stroke-dash-offset",
                      `${number(offset)}px`,
                      depth + 1,
                  ),
              ]
            : []),
        binding("stroke-line-cap", stroke.lineCap ?? "butt", depth + 1),
        binding("stroke-line-join", stroke.lineJoin ?? "miter", depth + 1),
        ...(stroke.miterLimit === undefined
            ? []
            : [binding("stroke-miter-limit", stroke.miterLimit, depth + 1)]),
        closeElement(depth),
    ];
    const clippedSource = (
        commands: string,
        dashes: readonly number[],
        offset: number,
        bounds?: { x: number; y: number; width: number; height: number },
    ): SlintLine[] => {
        if (bounds && (bounds.width <= 0 || bounds.height <= 0)) return [];
        if (!bounds || !stroke.lineCap || stroke.lineCap === "butt")
            return source(commands, dashes, offset);
        return [
            openElement("Rectangle", depth),
            binding("x", `${number(bounds.x)}px`, depth + 1),
            binding("y", `${number(bounds.y)}px`, depth + 1),
            binding("width", `${number(bounds.width)}px`, depth + 1),
            binding("height", `${number(bounds.height)}px`, depth + 1),
            binding("clip", true, depth + 1),
            ...source(commands, dashes, offset, depth + 1, {
                x: -expansion - bounds.x,
                y: -expansion - bounds.y,
            }),
            closeElement(depth),
        ];
    };
    const [tl, tr, br, bl] = radii;
    const inset = Math.max(0, weight - expansion);
    const endpoint = (radius: number) => radius || inset;
    const result: SlintLine[] = [];
    // Figma fits rectangular dashes independently to each edge and corner,
    // with half a dash on either end of each segment.
    const segment = (
        commands: string,
        originalLength: number,
        radialScale = 1,
        skipped = 0,
        clip?: { x: number; y: number; width: number; height: number },
    ) => {
        if (originalLength <= 0) return;
        const cycles = period > 0 ? Math.round(originalLength / period) : 0;
        const scale = cycles
            ? (originalLength / (cycles * period)) * radialScale
            : 1;
        result.push(
            ...clippedSource(
                commands,
                cycles ? pattern.map((length) => length * scale) : [],
                cycles ? (pattern[0] * scale) / 2 + skipped : 0,
                clip,
            ),
        );
    };
    segment(
        `M ${move(endpoint(tl), delta)} L ${move(node.width - endpoint(tr), delta)}`,
        node.width - tl - tr,
        1,
        tl === 0 ? inset : 0,
        {
            x: endpoint(tl),
            y: -expansion,
            width: node.width - endpoint(tl) - endpoint(tr),
            height: weight,
        },
    );
    segment(
        `M ${move(node.width - delta, endpoint(tr))} L ${move(node.width - delta, node.height - endpoint(br))}`,
        node.height - tr - br,
        1,
        tr === 0 ? inset : 0,
        {
            x: node.width - inset,
            y: endpoint(tr),
            width: weight,
            height: node.height - endpoint(tr) - endpoint(br),
        },
    );
    segment(
        `M ${move(node.width - endpoint(br), node.height - delta)} L ${move(endpoint(bl), node.height - delta)}`,
        node.width - br - bl,
        1,
        br === 0 ? inset : 0,
        {
            x: endpoint(bl),
            y: node.height - inset,
            width: node.width - endpoint(bl) - endpoint(br),
            height: weight,
        },
    );
    segment(
        `M ${move(delta, node.height - endpoint(bl))} L ${move(delta, endpoint(tl))}`,
        node.height - bl - tl,
        1,
        bl === 0 ? inset : 0,
        {
            x: -expansion,
            y: endpoint(tl),
            width: weight,
            height: node.height - endpoint(tl) - endpoint(bl),
        },
    );
    const corners = [
        [node.width - tr, tr, tr, -Math.PI / 2],
        [node.width - br, node.height - br, br, 0],
        [bl, node.height - bl, bl, Math.PI / 2],
        [tl, tl, tl, Math.PI],
    ];
    for (const [cx, cy, radius, angle] of corners) {
        const centerRadius = Math.max(0, radius - delta);
        if (radius === 0) {
            if (expansion > 0 || inset > 0) {
                const sx = cx === 0 ? -1 : 1;
                const sy = cy === 0 ? -1 : 1;
                const first = move(cx + sx * expansion, cy);
                const last = move(cx, cy + sy * expansion);
                const join = stroke.lineJoin ?? "miter";
                const corner =
                    join === "round" && expansion > 0
                        ? `A ${number(expansion)} ${number(expansion)} 0 0 ${sx * sy > 0 ? 1 : 0} ${last}`
                        : join === "bevel" ||
                            (stroke.miterLimit ?? 4) < Math.SQRT2
                          ? `L ${last}`
                          : `L ${move(cx + sx * expansion, cy + sy * expansion)} L ${last}`;
                result.push(
                    openElement("Path", depth),
                    binding("x", `${number(-expansion)}px`, depth + 1),
                    binding("y", `${number(-expansion)}px`, depth + 1),
                    binding("width", `${number(width)}px`, depth + 1),
                    binding("height", `${number(height)}px`, depth + 1),
                    binding("viewbox-width", width, depth + 1),
                    binding("viewbox-height", height, depth + 1),
                    binding(
                        "commands",
                        `"M ${move(cx - sx * inset, cy - sy * inset)} L ${move(cx + sx * expansion, cy - sy * inset)} L ${first} ${corner} L ${move(cx - sx * inset, cy + sy * expansion)} Z"`,
                        depth + 1,
                    ),
                    binding("fill", brush, depth + 1),
                    closeElement(depth),
                );
            }
            continue;
        }
        if (centerRadius === 0) continue;
        const end = angle + Math.PI / 2;
        segment(
            `M ${move(cx + centerRadius * Math.cos(angle), cy + centerRadius * Math.sin(angle))} A ${number(centerRadius)} ${number(centerRadius)} 0 0 1 ${move(cx + centerRadius * Math.cos(end), cy + centerRadius * Math.sin(end))}`,
            (radius * Math.PI) / 2,
            centerRadius / radius,
            0,
            {
                x: angle <= 0 ? cx : -expansion,
                y: angle === 0 || angle === Math.PI / 2 ? cy : -expansion,
                width: radius + expansion,
                height: radius + expansion,
            },
        );
    }
    return result;
}
