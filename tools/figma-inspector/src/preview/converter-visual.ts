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
    paintInBounds,
    type RenderContext,
    type NodePlacement,
} from "./converter-context";
import { normalizeSvg, utf8ToBase64 } from "../images";
import { dashedRectangleSource } from "./dashed-rectangle";
import type {
    SnapshotShadow,
    SnapshotAppearance,
    SnapshotFrameLikeNode,
    SnapshotRectangleNode,
    SnapshotNode,
    SnapshotPaintWithImage,
} from "../plugin/snapshot";

function opaquePaint(paint: SnapshotPaintWithImage): boolean {
    return (
        paint.kind !== "image" &&
        paint.opacity === 1 &&
        (paint.kind === "solid"
            ? paint.color.a === 1
            : paint.stops.every((stop) => stop.color.a === 1))
    );
}

function separateInnerBorder(appearance: SnapshotAppearance): boolean {
    const stroke = appearance.strokes[0];
    return (
        !!stroke &&
        appearance.shadows.some((shadow) => shadow.kind === "inner") &&
        (stroke.align === undefined || stroke.align === "INSIDE") &&
        stroke.dashPattern.length === 0 &&
        stroke.strokeTopWeight > 0 &&
        [
            stroke.strokeRightWeight,
            stroke.strokeBottomWeight,
            stroke.strokeLeftWeight,
        ].every((width) => width === stroke.strokeTopWeight)
    );
}

function separateShadowCaster(appearance: SnapshotAppearance): boolean {
    return (
        separateInnerBorder(appearance) &&
        !(appearance.fills.length === 1 && opaquePaint(appearance.fills[0]))
    );
}

export function shadowWarnings(
    node: SnapshotFrameLikeNode | SnapshotRectangleNode,
    context: RenderContext,
) {
    const outer = node.shadows.filter((shadow) => shadow.kind !== "inner");
    if (outer.length === 0) return;
    const opaque = node.fills.some(opaquePaint);
    if (!opaque && outer.some((shadow) => shadow.showBehind === false))
        context.warnings.push({
            severity: "warning",
            code: "SHADOW_TRANSPARENCY_APPROXIMATED",
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: "effects",
            message:
                "Native drop shadows can show through transparent paint; Figma's shadow knockout is approximated",
        });
    const stroke = node.strokes[0];
    if (
        node.fills.some((fill) => fill.kind === "image") ||
        node.fills.length > 1 ||
        (stroke &&
            (stroke.align === "outside" ||
                stroke.align === "center" ||
                stroke.dashPattern.length > 0 ||
                ![
                    stroke.strokeRightWeight,
                    stroke.strokeBottomWeight,
                    stroke.strokeLeftWeight,
                ].every((width) => width === stroke.strokeTopWeight)))
    )
        context.warnings.push({
            severity: "warning",
            code: "SHADOW_SHAPE_APPROXIMATED",
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: "effects",
            message:
                "Native drop shadows follow the rectangle's fill and uniform inside border; image, layered, and generated stroke shapes are approximated",
        });
    if (!opaque && (outer.length > 1 || separateInnerBorder(node)))
        context.warnings.push({
            severity: "warning",
            code:
                outer.length > 1
                    ? "MULTIPLE_SHADOW_PAINT_APPROXIMATED"
                    : "SHADOW_PAINT_APPROXIMATED",
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: "effects",
            message: "Separate native shadow casters repeat translucent paint",
        });
}

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

function shadowProperties(
    shadow: SnapshotShadow,
    depth: number,
    prefix = "drop-shadow",
): SlintLine[] {
    return [
        property(`${prefix}-color`, color(shadow.color), depth),
        ...lengthProperties(
            [
                [`${prefix}-blur`, shadow.blur],
                [`${prefix}-offset-x`, shadow.offsetX],
                [`${prefix}-offset-y`, shadow.offsetY],
                [`${prefix}-spread`, shadow.spread],
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
            !separateInnerBorder(appearance) &&
            stroke.dashPattern.length === 0 &&
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
    if (appearance.backdropBlur !== undefined)
        lines.push(
            property(
                "backdrop-blur",
                `${number(appearance.backdropBlur)}px`,
                depth,
            ),
        );
    lines.push(...cornerRadiusSource(appearance.cornerRadii, depth));
    if (appearance.clipsContent && !suppressClip)
        lines.push(property("clip", "true", depth));
    const outerShadows = appearance.shadows.filter(
        (shadow) => shadow.kind !== "inner",
    );
    const shadow = outerShadows[0];
    if (
        shadow !== undefined &&
        outerShadows.length === 1 &&
        !separateShadowCaster(appearance)
    )
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
    const uniform = weights.every((weight) => weight === weights[0]);
    if (uniform && stroke.dashPattern.length > 0)
        return dashedRectangleSource(node, stroke, depth);

    if (
        (stroke.align === undefined || stroke.align === "INSIDE") &&
        weights.every((weight) => weight === weights[0])
    )
        return separateInnerBorder(node)
            ? [
                  openElement("Rectangle", depth),
                  property("x", "0px", depth + 1),
                  ...cornerRadiusSource(node.cornerRadii, depth + 1),
                  property(
                      "border-width",
                      `${number(weights[0])}px`,
                      depth + 1,
                  ),
                  property("border-color", paint(stroke.paint), depth + 1),
                  closeElement(depth),
              ]
            : [];
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
        property(
            "fill",
            paintInBounds(stroke.paint, node, {
                x: -expandLeft,
                y: -expandTop,
                width: outerWidth,
                height: outerHeight,
            }),
            depth + 1,
        ),
        closeElement(depth),
    ];
}

export function fillLayersSource(
    appearance: SnapshotAppearance,
    depth: number,
): SlintLine[] {
    if (
        appearance.fills.length <= 1 &&
        !(
            separateShadowCaster(appearance) &&
            appearance.shadows.some((shadow) => shadow.kind !== "inner")
        )
    )
        return [];
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
    const outerShadows = node.shadows.filter(
        (shadow) => shadow.kind !== "inner",
    );
    if (
        outerShadows.length === 0 ||
        (outerShadows.length === 1 && !separateShadowCaster(node))
    )
        return [];
    if (node.clipsContent)
        context.warnings.push({
            severity: "warning",
            code:
                outerShadows.length > 1
                    ? "MULTIPLE_SHADOW_CLIPPING_APPROXIMATED"
                    : "SHADOW_CLIPPING_APPROXIMATED",
            nodeId: node.id,
            nodeName: node.name,
            propertyPath: "effects",
            message:
                "Separate shadow casters are subject to the node's content clip in Slint",
        });
    const fill = node.fills.find((candidate) => candidate.kind !== "image");
    return outerShadows.flatMap((shadow) => [
        openElement("Rectangle", depth),
        property("x", "0px", depth + 1),
        ...appearanceSource(
            {
                ...node,
                fills: fill === undefined ? [] : [fill],
                shadows: [],
                clipsContent: false,
                backdropBlur: undefined,
            },
            depth + 1,
        ),
        ...shadowProperties(shadow, depth + 1),
        closeElement(depth),
    ]);
}

export function innerShadowSource(
    appearance: SnapshotAppearance,
    depth: number,
): SlintLine[] {
    return appearance.shadows
        .filter((shadow) => shadow.kind === "inner")
        .flatMap((shadow) => [
            openElement("Rectangle", depth),
            property("x", "0px", depth + 1),
            ...cornerRadiusSource(appearance.cornerRadii, depth + 1),
            ...shadowProperties(shadow, depth + 1, "inner-shadow"),
            closeElement(depth),
        ]);
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
        fill.scaleMode === "STRETCH"
            ? "fill"
            : fill.scaleMode === "FIT"
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
        ...(fill.scaleMode === "STRETCH"
            ? [property("image-fit", "fill", imagePropertiesDepth)]
            : fit === "fill" || fill.scaleMode === "TILE"
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
    const paintBounds = node.raster?.bounds ?? node.svgBounds;
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
