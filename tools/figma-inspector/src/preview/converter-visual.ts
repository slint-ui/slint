// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import {
    binding as property,
    openElement,
    closeElement,
    type SlintLine,
} from "./slint-ir";
import {
    color,
    number,
    lengthProperties,
    paint,
    type RenderContext,
    type NodePlacement,
} from "./converter-context";
import { normalizeSvg, utf8ToBase64 } from "../images";
import type {
    SnapshotShadow,
    SnapshotAppearance,
    SnapshotFrameLikeNode,
    SnapshotRectangleNode,
    SnapshotNode,
} from "../plugin/snapshot";

export function cornerRadiusSource(
    radii: SnapshotAppearance["cornerRadii"],
    depth: number,
): SlintLine[] {
    if (radii.every((radius) => radius === radii[0]))
        return lengthProperties([["border-radius", radii[0]]], depth);
    return lengthProperties(
        [
            ["border-top-left-radius", radii[0]],
            ["border-top-right-radius", radii[1]],
            ["border-bottom-right-radius", radii[2]],
            ["border-bottom-left-radius", radii[3]],
        ],
        depth,
    );
}

function shadowProperties(shadow: SnapshotShadow, depth: number): SlintLine[] {
    return [
        property("drop-shadow-color", color(shadow.color), depth),
        ...lengthProperties(
            [
                ["drop-shadow-blur", shadow.blur],
                ["drop-shadow-offset-x", shadow.offsetX],
                ["drop-shadow-offset-y", shadow.offsetY],
                ["drop-shadow-spread", shadow.spread],
            ],
            depth,
        ),
    ];
}

export function appearanceSource(
    appearance: SnapshotAppearance,
    depth: number,
    suppressClip = false,
): SlintLine[] {
    const lines: SlintLine[] = [];
    const fill = appearance.fills[0];
    if (
        appearance.fills.length === 1 &&
        fill !== undefined &&
        fill.kind !== "image"
    )
        lines.push(property("background", paint(fill), depth));
    const stroke = appearance.strokes[0];
    if (stroke !== undefined) {
        const uniform =
            stroke.strokeTopWeight === stroke.strokeRightWeight &&
            stroke.strokeRightWeight === stroke.strokeBottomWeight &&
            stroke.strokeBottomWeight === stroke.strokeLeftWeight;
        if (
            uniform &&
            (stroke.align === undefined || stroke.align === "INSIDE") &&
            stroke.strokeTopWeight !== 0
        ) {
            lines.push(
                property(
                    "border-width",
                    `${number(stroke.strokeTopWeight)}px`,
                    depth,
                ),
                property("border-color", paint(stroke.paint), depth),
            );
        }
    }
    lines.push(...cornerRadiusSource(appearance.cornerRadii, depth));
    if (appearance.clipsContent && !suppressClip)
        lines.push(property("clip", "true", depth));
    const outerShadows = appearance.shadows.filter(
        (shadow) => shadow.kind !== "inner",
    );
    const shadow = outerShadows[0];
    if (shadow !== undefined && outerShadows.length === 1)
        lines.push(...shadowProperties(shadow, depth));
    return lines;
}

export function strokeLayersSource(
    node: SnapshotFrameLikeNode | SnapshotRectangleNode,
    depth: number,
): SlintLine[] {
    const stroke = node.strokes[0];
    if (stroke === undefined) return [];
    const weights = [
        stroke.strokeTopWeight,
        stroke.strokeRightWeight,
        stroke.strokeBottomWeight,
        stroke.strokeLeftWeight,
    ];
    if (
        (stroke.align === undefined || stroke.align === "INSIDE") &&
        weights.every((weight) => weight === weights[0])
    )
        return [];
    const [top, right, bottom, left] = weights;
    const expansion =
        stroke.align === "center" ? 0.5 : stroke.align === "outside" ? 1 : 0;
    const expandTop = top * expansion;
    const expandRight = right * expansion;
    const expandBottom = bottom * expansion;
    const expandLeft = left * expansion;
    const outerWidth = node.width + expandLeft + expandRight;
    const outerHeight = node.height + expandTop + expandBottom;
    const radii = node.cornerRadii.map((radius) =>
        Math.max(0, Math.min(radius, node.width / 2, node.height / 2)),
    );
    const corners = [
        [radii[0] + expandLeft, radii[0] + expandTop],
        [radii[1] + expandRight, radii[1] + expandTop],
        [radii[2] + expandRight, radii[2] + expandBottom],
        [radii[3] + expandLeft, radii[3] + expandBottom],
    ] as const;
    const [topLeft, topRight, bottomRight, bottomLeft] = corners;
    const arc = (
        radiusX: number,
        radiusY: number,
        x: number,
        y: number,
        sweep: 0 | 1,
    ): string =>
        radiusX > 0 && radiusY > 0
            ? `A ${number(radiusX)} ${number(radiusY)} 0 0 ${sweep} ${number(x)} ${number(y)}`
            : `L ${number(x)} ${number(y)}`;
    const outer = [
        `M ${number(topLeft[0])} 0`,
        `H ${number(outerWidth - topRight[0])}`,
        arc(topRight[0], topRight[1], outerWidth, topRight[1], 1),
        `V ${number(outerHeight - bottomRight[1])}`,
        arc(
            bottomRight[0],
            bottomRight[1],
            outerWidth - bottomRight[0],
            outerHeight,
            1,
        ),
        `H ${number(bottomLeft[0])}`,
        arc(bottomLeft[0], bottomLeft[1], 0, outerHeight - bottomLeft[1], 1),
        `V ${number(topLeft[1])}`,
        arc(topLeft[0], topLeft[1], topLeft[0], 0, 1),
        "Z",
    ].join(" ");
    const innerLeft = left;
    const innerTop = top;
    const innerRight = Math.max(left, outerWidth - right);
    const innerBottom = Math.max(top, outerHeight - bottom);
    const innerRadii = [
        [Math.max(0, topLeft[0] - left), Math.max(0, topLeft[1] - top)],
        [Math.max(0, topRight[0] - right), Math.max(0, topRight[1] - top)],
        [
            Math.max(0, bottomRight[0] - right),
            Math.max(0, bottomRight[1] - bottom),
        ],
        [
            Math.max(0, bottomLeft[0] - left),
            Math.max(0, bottomLeft[1] - bottom),
        ],
    ] as const;
    const [innerTopLeft, innerTopRight, innerBottomRight, innerBottomLeft] =
        innerRadii;
    const inner =
        innerRight > innerLeft && innerBottom > innerTop
            ? [
                  `M ${number(innerLeft)} ${number(innerTop + innerTopLeft[1])}`,
                  `V ${number(innerBottom - innerBottomLeft[1])}`,
                  arc(
                      innerBottomLeft[0],
                      innerBottomLeft[1],
                      innerLeft + innerBottomLeft[0],
                      innerBottom,
                      0,
                  ),
                  `H ${number(innerRight - innerBottomRight[0])}`,
                  arc(
                      innerBottomRight[0],
                      innerBottomRight[1],
                      innerRight,
                      innerBottom - innerBottomRight[1],
                      0,
                  ),
                  `V ${number(innerTop + innerTopRight[1])}`,
                  arc(
                      innerTopRight[0],
                      innerTopRight[1],
                      innerRight - innerTopRight[0],
                      innerTop,
                      0,
                  ),
                  `H ${number(innerLeft + innerTopLeft[0])}`,
                  arc(
                      innerTopLeft[0],
                      innerTopLeft[1],
                      innerLeft,
                      innerTop + innerTopLeft[1],
                      0,
                  ),
                  "Z",
              ].join(" ")
            : "";
    return [
        openElement("Path", depth),
        property("x", `${number(-expandLeft)}px`, depth + 1),
        property("y", `${number(-expandTop)}px`, depth + 1),
        property("width", `${number(outerWidth)}px`, depth + 1),
        property("height", `${number(outerHeight)}px`, depth + 1),
        property(
            "commands",
            `"${outer}${inner ? ` ${inner}` : ""}"`,
            depth + 1,
        ),
        property("fill", paint(stroke.paint), depth + 1),
        closeElement(depth),
    ];
}

export function fillLayersSource(
    appearance: SnapshotAppearance,
    depth: number,
): SlintLine[] {
    if (appearance.fills.length <= 1) return [];
    return appearance.fills.flatMap((fill) => {
        if (fill.kind === "image") {
            return imageFillSource({ ...appearance, fills: [fill] }, depth);
        }
        return [
            openElement("Rectangle", depth),
            property("background", paint(fill), depth + 1),
            ...cornerRadiusSource(appearance.cornerRadii, depth + 1),
            closeElement(depth),
        ];
    });
}

export function shadowLayersSource(
    node: SnapshotFrameLikeNode | SnapshotRectangleNode,
    context: RenderContext,
    depth: number,
): SlintLine[] {
    const appearance = node;
    const outerShadows = appearance.shadows.filter(
        (shadow) => shadow.kind !== "inner",
    );
    if (outerShadows.length <= 1) return [];
    if (appearance.clipsContent)
        context.warnings.push({
            severity: "warning",
            code: "MULTIPLE_SHADOW_CLIPPING_APPROXIMATED",
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: "effects",
            message:
                "Multiple shadow layers are subject to the node's content clip in Slint",
        });
    const fill = appearance.fills.find(
        (candidate) => candidate.kind !== "image",
    );
    return outerShadows.flatMap((shadow) => [
        openElement("Rectangle", depth),
        ...(fill === undefined
            ? []
            : [property("background", paint(fill), depth + 1)]),
        ...(fill === undefined
            ? []
            : cornerRadiusSource(appearance.cornerRadii, depth + 1)),
        ...shadowProperties(shadow, depth + 1),
        closeElement(depth),
    ]);
}

export function innerShadowSource(
    appearance: SnapshotAppearance,
    depth: number,
): SlintLine[] {
    const shadows = appearance.shadows.filter(
        (shadow) => shadow.kind === "inner",
    );
    if (!shadows.length) return [];
    const lines = [
        openElement("Rectangle", depth),
        property("clip", true, depth + 1),
        ...cornerRadiusSource(appearance.cornerRadii, depth + 1),
    ];
    // Separable Gaussian edge profiles approximate a rectangular inset shadow.
    // The positioned edge strips do not contribute intrinsic layout size, so
    // this wrapper can use Rectangle's default fill geometry.
    const tail = (z: number): number => {
        const x = Math.abs(z) / Math.SQRT2;
        const t = 1 / (1 + 0.3275911 * x);
        const erf =
            1 -
            ((((1.061405429 * t - 1.453152027) * t + 1.421413741) * t -
                0.284496736) *
                t +
                0.254829592) *
                t *
                Math.exp(-x * x);
        return (1 - (z < 0 ? -erf : erf)) / 2;
    };
    for (const shadow of shadows) {
        const sigma = shadow.blur / 2;
        const sides = [
            {
                shift: shadow.offsetX,
                angle: 90,
                axis: "width",
                anchor: "x",
                far: false,
            },
            {
                shift: -shadow.offsetX,
                angle: 270,
                axis: "width",
                anchor: "x",
                far: true,
            },
            {
                shift: shadow.offsetY,
                angle: 180,
                axis: "height",
                anchor: "y",
                far: false,
            },
            {
                shift: -shadow.offsetY,
                angle: 0,
                axis: "height",
                anchor: "y",
                far: true,
            },
        ];
        for (const side of sides) {
            const boundary = shadow.spread + side.shift;
            const span = Math.max(0, boundary + 3 * sigma);
            if (!span) continue;
            const stops = Array.from(
                { length: 13 },
                (_, i) =>
                    `${color(shadow.color, sigma ? tail(((span * i) / 12 - boundary) / sigma) : 1)} ${number((i * 100) / 12)}%`,
            ).join(", ");
            lines.push(
                openElement("Rectangle", depth + 1),
                property(
                    "x",
                    side.far && side.anchor === "x"
                        ? `parent.width - ${number(span)}px`
                        : "0px",
                    depth + 2,
                ),
                property(
                    "y",
                    side.far && side.anchor === "y"
                        ? `parent.height - ${number(span)}px`
                        : "0px",
                    depth + 2,
                ),
            );
            lines.push(
                property(side.axis, `${number(span)}px`, depth + 2),
                property(
                    "background",
                    sigma
                        ? `@linear-gradient(${side.angle}deg, ${stops})`
                        : color(shadow.color),
                    depth + 2,
                ),
                closeElement(depth + 1),
            );
        }
    }
    lines.push(closeElement(depth));
    return lines;
}

export function imageFillSource(
    appearance: SnapshotAppearance,
    depth: number,
): SlintLine[] {
    const fill = appearance.fills[0];
    if (
        appearance.fills.length !== 1 ||
        fill === undefined ||
        fill.kind !== "image"
    )
        return [];
    const rounded = appearance.cornerRadii.some((radius) => radius !== 0);
    const wrapperDepth = rounded ? depth + 1 : depth;
    const imageDepth = rounded ? wrapperDepth + 1 : wrapperDepth;
    const imagePropertiesDepth = imageDepth + 1;
    const tileScale = fill.scaleMode === "TILE" ? (fill.tileScale ?? 1) : 1;
    const fit =
        fill.scaleMode === "FIT"
            ? "contain"
            : fill.scaleMode === "CROP" && fill.crop !== undefined
              ? "fill"
              : "cover";
    const lines = [
        ...(rounded
            ? [
                  openElement("Rectangle", depth),
                  // Decorative fills must not contribute intrinsic layout
                  // size: the image already derives its size from this node.
                  // Slint excludes a child with either position binding. One
                  // is sufficient; y already defaults to zero at full height.
                  property("x", "0px", wrapperDepth),
                  property("clip", "true", wrapperDepth),
                  ...cornerRadiusSource(appearance.cornerRadii, wrapperDepth),
              ]
            : []),
        openElement("Image", imageDepth),
        ...(tileScale === 1
            ? []
            : [
                  property("x", "0px", imagePropertiesDepth),
                  property("y", "0px", imagePropertiesDepth),
              ]),
        property(
            "width",
            tileScale === 1 ? "100%" : `parent.width / ${tileScale}`,
            imagePropertiesDepth,
        ),
        property(
            "height",
            tileScale === 1 ? "100%" : `parent.height / ${tileScale}`,
            imagePropertiesDepth,
        ),
        property("source", paint(fill), imagePropertiesDepth),
        ...(fit === "fill" || fill.scaleMode === "TILE"
            ? []
            : [property("image-fit", fit, imagePropertiesDepth)]),
    ];
    if (fill.scaleMode === "TILE") {
        lines.push(
            property("horizontal-tiling", "repeat", imagePropertiesDepth),
            property("vertical-tiling", "repeat", imagePropertiesDepth),
            property("horizontal-alignment", "left", imagePropertiesDepth),
            property("vertical-alignment", "top", imagePropertiesDepth),
        );
        if (tileScale !== 1)
            lines.push(
                property(
                    "transform-origin",
                    "{ x: 0px, y: 0px }",
                    imagePropertiesDepth,
                ),
                property(
                    "transform-scale",
                    String(tileScale),
                    imagePropertiesDepth,
                ),
            );
    }
    if (fill.opacity !== 1)
        lines.push(property("opacity", fill.opacity, imagePropertiesDepth));
    if (fill.scaleMode === "CROP" && fill.crop !== undefined) {
        const [x, y, width, height] = fill.crop;
        lines.push(
            property(
                "source-clip-x",
                Math.round(x * fill.intrinsicWidth),
                imagePropertiesDepth,
            ),
            property(
                "source-clip-y",
                Math.round(y * fill.intrinsicHeight),
                imagePropertiesDepth,
            ),
            property(
                "source-clip-width",
                Math.max(1, Math.round(width * fill.intrinsicWidth)),
                imagePropertiesDepth,
            ),
            property(
                "source-clip-height",
                Math.max(1, Math.round(height * fill.intrinsicHeight)),
                imagePropertiesDepth,
            ),
        );
    }
    if (rounded) lines.push(closeElement(wrapperDepth), closeElement(depth));
    else lines.push(closeElement(depth));
    return lines;
}

export function visualSource(
    node: Extract<SnapshotNode, { kind: "svg" }>,
    depth: number,
    { flexItem, parent }: NodePlacement,
): SlintLine[] | undefined {
    const lines: SlintLine[] = [];
    const paintBounds = node.paintBounds ?? node.raster?.bounds;
    const imageData =
        node.raster !== undefined
            ? `data:image/png;base64,${node.raster.data}`
            : node.svg !== undefined
              ? `data:image/svg+xml;base64,${utf8ToBase64(normalizeSvg(node.svg))}`
              : undefined;
    if (imageData === undefined) return undefined;
    if (paintBounds) {
        lines.push(
            openElement("Image", depth + 1),
            property("x", `${number(paintBounds.x)}px`, depth + 2),
            property("y", `${number(paintBounds.y)}px`, depth + 2),
            property("width", `${number(paintBounds.width)}px`, depth + 2),
            property("height", `${number(paintBounds.height)}px`, depth + 2),
            property("source", `@image-url("${imageData}")`, depth + 2),
            property("image-fit", "fill", depth + 2),
            closeElement(depth + 1),
        );
    } else {
        lines.push(property("source", `@image-url("${imageData}")`, depth + 1));
        if (node.raster !== undefined)
            // Figma's export includes the full node box. Fill maps every
            // captured pixel back onto that same logical box, preserving
            // transparent padding and avoiding density-dependent centering.
            lines.push(
                property("image-fit", "fill", depth + 1),
                property("image-rendering", "pixelated", depth + 1),
            );
        else if (
            flexItem ||
            parent === undefined ||
            (node.layoutSizingHorizontal === null &&
                node.layoutSizingVertical === null)
        )
            lines.push(property("image-fit", "fill", depth + 1));
    }
    return lines;
}
