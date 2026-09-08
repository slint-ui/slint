// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import {
    bytesToBase64,
    normalizeSvgToNodeBounds,
    isPngByteArray,
    pngDimensions,
    imageDimensions,
} from "../images";
import {
    color,
    isMixed,
    number,
    problem,
    CompatibilityError,
    type MaterializedNode,
    type NormalizationContext,
    type NormalizedGeometry,
    type NodeResult,
} from "./normalization-context";
import type { ImageResolver } from "./normalize";
import type {
    Diagnostic,
    SnapshotImage,
    SnapshotPaint,
    SnapshotRaster,
    SnapshotTransform,
} from "./snapshot";
type MixedValue = unknown;

type VisualExport = {
    readonly svg?: string;
    readonly raster?: SnapshotRaster;
    readonly svgError?: unknown;
    readonly pngError?: unknown;
};

export async function exportVisual(
    node: MaterializedNode,
    exportSvgNode: NormalizationContext["exportSvgNode"],
    exportPngNode?: NormalizationContext["exportPngNode"],
    exportScale = 1,
    encode: (bytes: Uint8Array) => string = bytesToBase64,
): Promise<VisualExport> {
    const bounds = node.sourceRasterBounds;
    const pngExporter =
        node.sourceSvgBounds && !bounds ? undefined : exportPngNode;
    const pngRequested = pngExporter !== undefined;
    const [svgResult, pngResult] = await Promise.allSettled([
        Promise.resolve().then(() => exportSvgNode(node)),
        pngExporter === undefined
            ? Promise.resolve<Uint8Array | undefined>(undefined)
            : Promise.resolve().then(() => pngExporter(node, exportScale)),
    ]);
    const svg =
        svgResult.status === "fulfilled" && typeof svgResult.value === "string"
            ? svgResult.value
            : undefined;
    let raster: SnapshotRaster | undefined;
    let pngError: unknown;
    if (pngRequested) {
        if (pngResult.status === "rejected") pngError = pngResult.reason;
        else {
            const bytes = pngResult.value;
            if (bytes === undefined)
                pngError = new Error("export returned no PNG bytes");
            else if (!isPngByteArray(bytes))
                pngError = new Error("export returned invalid PNG bytes");
            else if (bytes.length === 0)
                pngError = new Error("export returned empty PNG bytes");
            else {
                const dimensions = pngDimensions(bytes);
                if (!dimensions)
                    pngError = new Error("export returned invalid PNG header");
                else
                    raster = {
                        ...(bounds ? { bounds } : {}),
                        data: encode(bytes),
                        exportScale,
                        pixelWidth: dimensions.width,
                        pixelHeight: dimensions.height,
                    };
            }
        }
    }
    return {
        ...(svg === undefined ? {} : { svg }),
        ...(raster === undefined ? {} : { raster }),
        ...(svgResult.status === "rejected"
            ? { svgError: svgResult.reason }
            : {}),
        ...(pngError === undefined ? {} : { pngError }),
    };
}

function validSvgDocument(value: string | undefined): value is string {
    return (
        value !== undefined &&
        (/^<svg(?:\s[^>]*)?>[\s\S]*<\/svg\s*>$/iu.test(value) ||
            /^<svg(?:\s[^>]*)?\/>$/iu.test(value))
    );
}

function exportFailureMessage(error: unknown, fallback: string): string {
    return error instanceof Error
        ? error.message
        : error === undefined
          ? fallback
          : String(error);
}

function visualExportWarnings(
    node: MaterializedNode,
    exported: VisualExport,
    validSvg: boolean,
): Diagnostic[] {
    const warnings: Diagnostic[] = [];
    if (
        !validSvg &&
        exported.raster !== undefined &&
        !("sourceSvgOmitted" in node && node.sourceSvgOmitted === true)
    )
        warnings.push({
            ...problem(
                "SVG_EXPORT_FALLBACK",
                node,
                `Figma SVG export was unavailable for "${node.name}" (${node.id}): ${exportFailureMessage(exported.svgError, "export returned malformed SVG")}; using Figma raster provenance`,
                "exportAsync",
            ),
            severity: "warning",
        });
    return warnings;
}

function transform(
    value: Transform,
    node: MaterializedNode,
): SnapshotTransform {
    if (
        !Array.isArray(value) ||
        value.length !== 2 ||
        value.some(
            (row) =>
                !Array.isArray(row) ||
                row.length !== 3 ||
                row.some((entry) => !number(entry)),
        )
    ) {
        throw new CompatibilityError(
            `Invalid gradient transform on ${node.name}`,
        );
    }
    return [
        [value[0][0], value[0][1], value[0][2]],
        [value[1][0], value[1][1], value[1][2]],
    ];
}

async function paint(
    value: Paint,
    node: MaterializedNode,
    imageResolver: ImageResolver,
): Promise<SnapshotPaint | SnapshotImage> {
    if (value.visible === false) {
        throw new Error("invisible paints must be filtered before conversion");
    }
    if (!number(value.opacity) || value.opacity < 0 || value.opacity > 1)
        throw new CompatibilityError("paint opacity must be between 0 and 1");
    const opacity = value.opacity;
    if (value.type === "SOLID") {
        return { kind: "solid", color: color(value.color, node), opacity };
    }
    if (value.type === "GRADIENT_LINEAR") {
        if (value.gradientStops.length < 2)
            throw new CompatibilityError(
                "linear gradients require at least two stops",
            );
        return {
            kind: "linear-gradient",
            transform: transform(value.gradientTransform, node),
            stops: value.gradientStops.map((stop) => {
                if (
                    !number(stop.position) ||
                    stop.position < 0 ||
                    stop.position > 1
                )
                    throw new CompatibilityError(
                        "gradient stop position must be between 0 and 1",
                    );
                return {
                    position: stop.position,
                    color: color(stop.color, node),
                };
            }),
            opacity,
        };
    }
    if (value.type === "IMAGE") {
        if (value.imageHash === null)
            throw new CompatibilityError(
                "image paint does not contain an image hash",
            );
        const image = await imageResolver(value.imageHash);
        if (image === undefined)
            throw new CompatibilityError(
                `image ${value.imageHash} could not be retrieved`,
            );
        const bytes = image.bytes;
        const dimensions =
            image.width !== undefined && image.height !== undefined
                ? { width: image.width, height: image.height }
                : imageDimensions(bytes);
        if (!dimensions)
            throw new CompatibilityError(
                "Image dimensions unavailable in capture and image bytes",
            );
        let mimeType: SnapshotImage["mimeType"];
        if (
            bytes.length >= 8 &&
            bytes[0] === 0x89 &&
            bytes[1] === 0x50 &&
            bytes[2] === 0x4e &&
            bytes[3] === 0x47
        )
            mimeType = "image/png";
        else if (
            bytes.length >= 3 &&
            bytes[0] === 0xff &&
            bytes[1] === 0xd8 &&
            bytes[2] === 0xff
        )
            mimeType = "image/jpeg";
        else if (
            bytes.length >= 6 &&
            bytes[0] === 0x47 &&
            bytes[1] === 0x49 &&
            bytes[2] === 0x46
        )
            mimeType = "image/gif";
        else
            throw new CompatibilityError(
                "image bytes use an unsupported format",
            );
        // Figma maps normalized node coordinates into source-image coordinates.
        // For an axis-aligned crop, translation is the source origin and scale
        // is the source extent; inverting it restores cropped-out image borders.
        const crop = value.imageTransform;
        const cropRect =
            crop === undefined
                ? undefined
                : Math.abs(crop[0][1]) < 1e-6 &&
                    Math.abs(crop[1][0]) < 1e-6 &&
                    crop[0][0] !== 0 &&
                    crop[1][1] !== 0
                  ? (() => {
                        const x = Math.max(0, Math.min(1, crop[0][2]));
                        const y = Math.max(0, Math.min(1, crop[1][2]));
                        const width = Math.min(Math.max(0, crop[0][0]), 1 - x);
                        const height = Math.min(Math.max(0, crop[1][1]), 1 - y);
                        return width > 0 && height > 0
                            ? ([x, y, width, height] as const)
                            : undefined;
                    })()
                  : undefined;
        return {
            kind: "image",
            mimeType,
            data: image.base64 ?? bytesToBase64(bytes),
            intrinsicWidth: dimensions.width,
            intrinsicHeight: dimensions.height,
            scaleMode: value.scaleMode,
            ...(value.filters?.saturation === -1 ? { grayscale: true } : {}),
            ...(value.scaleMode === "TILE"
                ? {
                      tileScale:
                          number(value.scalingFactor) && value.scalingFactor > 0
                              ? value.scalingFactor
                              : 1,
                  }
                : {}),
            ...(cropRect === undefined
                ? {}
                : {
                      crop: cropRect,
                  }),
            opacity,
        };
    }
    throw new CompatibilityError(`Unsupported paint type ${value.type}`);
}

export async function visiblePaints(
    value: ReadonlyArray<Paint> | symbol,
    node: MaterializedNode,
    mixedValue: MixedValue,
    property: string,
    imageResolver: ImageResolver,
): Promise<{
    paints: (SnapshotPaint | SnapshotImage)[];
    warnings: Diagnostic[];
}> {
    if (isMixed(value, mixedValue)) {
        return {
            paints: [],
            warnings: [
                {
                    ...problem(
                        "MIXED_PAINT_APPROXIMATED",
                        node,
                        `${property} contains mixed values and was omitted`,
                        property,
                    ),
                    severity: "warning",
                },
            ],
        };
    }
    const sourcePaints = value as ReadonlyArray<Paint>;
    const visible = sourcePaints.filter((item) => item.visible !== false);
    const paints: (SnapshotPaint | SnapshotImage)[] = [];
    const warnings: Diagnostic[] = [];
    for (const item of visible) {
        try {
            paints.push(await paint(item, node, imageResolver));
            if (item.type === "IMAGE" && item.imageHash) {
                const image = await imageResolver(item.imageHash);
                if (
                    image &&
                    (image.width === undefined || image.height === undefined)
                ) {
                    warnings.push({
                        ...problem(
                            "IMAGE_DIMENSIONS_RECOVERED",
                            node,
                            `${image.sizeError ?? "Image dimensions unavailable"}; intrinsic dimensions recovered from image bytes`,
                            property,
                        ),
                        severity: "warning",
                    });
                }
            }
        } catch (error) {
            if (!(error instanceof CompatibilityError)) throw error;
            const paintType =
                typeof item === "object" && item !== null && "type" in item
                    ? String(item.type)
                    : "unknown";
            const code =
                paintType === "SOLID" ||
                paintType === "GRADIENT_LINEAR" ||
                paintType === "IMAGE"
                    ? "UNSUPPORTED_PAINT"
                    : "UNSUPPORTED_PAINT_IGNORED";
            const diagnostic = problem(
                code,
                node,
                error instanceof Error ? error.message : String(error),
                property,
            );
            warnings.push({
                ...diagnostic,
                message: `${diagnostic.message}; paint omitted`,
                severity: "warning",
            });
        }
    }
    if (visible.length > 1)
        warnings.push({
            ...problem(
                "MULTIPLE_VISIBLE_PAINTS_APPROXIMATED",
                node,
                property === "strokes"
                    ? "strokes contains multiple visible paints; only the first supported stroke paint is rendered"
                    : `${property} contains multiple visible paints; all supported layers are preserved`,
                property,
            ),
            severity: "warning",
        });
    return { paints, warnings };
}

/** Resolve captured exports without sharing context-sensitive pixels between nodes. */
export async function normalizeVisual(
    node: MaterializedNode,
    base: NormalizedGeometry,
    context: NormalizationContext,
    captureWarnings: Diagnostic[],
    policy: "text" | "vector",
): Promise<NodeResult> {
    try {
        const exported = await exportVisual(
            node,
            context.exportSvgNode,
            context.exportPngNode,
            context.exportScale,
            context.assets.encode,
        );
        if (exported.pngError !== undefined)
            return {
                error: problem(
                    "PNG_EXPORT_FAILED",
                    node,
                    `Failed to export PNG: ${exportFailureMessage(exported.pngError, "PNG unavailable")}`,
                    "exportAsync",
                ),
            };
        const paintBounds =
            policy === "vector" ? node.sourceSvgBounds : undefined;
        const svg =
            typeof exported.svg === "string"
                ? paintBounds
                    ? exported.svg.trim()
                    : normalizeSvgToNodeBounds(
                          exported.svg,
                          base.width,
                          base.height,
                      )
                : "";
        const validSvg =
            validSvgDocument(svg) &&
            (policy !== "text" || !/<text(?:\s|>)/iu.test(svg));
        if (!validSvg && exported.raster === undefined) {
            if (policy === "text")
                return {
                    error: problem(
                        exported.svgError === undefined
                            ? "TEXT_SVG_EXPORT_INVALID"
                            : "SVG_EXPORT_FAILED",
                        node,
                        exported.svgError === undefined
                            ? "Text image export must be a valid outlined SVG without <text> elements"
                            : `Failed to export node "${node.name}" (${node.id}): ${exportFailureMessage(exported.svgError, "SVG export failed")}`,
                        "exportAsync",
                    ),
                };
            throw (
                exported.svgError ??
                new CompatibilityError(
                    "export returned an empty or malformed SVG",
                )
            );
        }
        const warnings = [
            ...captureWarnings,
            ...visualExportWarnings(node, exported, validSvg),
        ];
        if (
            policy === "vector" &&
            (node.type === "CONNECTOR" || node.type === "SHAPE_WITH_TEXT")
        )
            warnings.push({
                ...problem(
                    "SVG_FLATTENED_APPROXIMATED",
                    node,
                    "This visual node was flattened through Figma SVG export",
                    "exportAsync",
                ),
                severity: "warning",
            });
        return {
            node: {
                ...base,
                kind: "svg",
                ...(paintBounds && validSvg ? { paintBounds } : {}),
                sourceType: policy === "text" ? "TEXT" : node.type,
                ...(validSvg ? { svg } : {}),
                ...(exported.raster === undefined
                    ? {}
                    : { raster: exported.raster }),
            },
            warnings,
        };
    } catch (error) {
        if (!(error instanceof CompatibilityError)) throw error;
        return {
            error: problem(
                "SVG_EXPORT_FAILED",
                node,
                `Failed to export node "${node.name}" (${node.id}): ${error instanceof Error ? error.message : String(error)}`,
                "exportAsync",
            ),
        };
    }
}
