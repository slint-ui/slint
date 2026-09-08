// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import {
    type VariableLibrary,
    validateVariableLibrary,
} from "./variable-library";
import { type ComponentLibrary, validateComponentLibrary } from "./components";
import { validVisualBounds, type VisualBounds } from "./source";

/**
 * The only data contract shared by the Figma sandbox, the converter, and the
 * preview. Nothing in this file imports Figma's plugin types so snapshots can
 * be used in Node and browser tests without a Figma host.
 */

export const SNAPSHOT_SCHEMA_VERSION = 8 as const;

export type SnapshotSizing = "fixed" | "hug" | "fill";

export type SnapshotAutoLayout = {
    readonly direction: "horizontal" | "vertical";
    readonly paddingLeft: number;
    readonly paddingRight: number;
    readonly paddingTop: number;
    readonly paddingBottom: number;
    readonly itemSpacing: number;
    readonly wrap: boolean;
    readonly counterAxisSpacing: number;
    readonly counterAxisAlignContent:
        | "start"
        | "center"
        | "end"
        | "space-between"
        | "stretch";
    readonly primaryAlignment: "start" | "center" | "end" | "space-between";
    readonly counterAlignment: "start" | "center" | "end";
    /** Emit children in reverse paint order while retaining their layout order. */
    readonly reversePaintOrder?: true;
};

export type SnapshotColor = {
    readonly r: number;
    readonly g: number;
    readonly b: number;
    readonly a: number;
};

export type SnapshotTransform = readonly [
    readonly [number, number, number],
    readonly [number, number, number],
];

export type SnapshotPaint =
    | {
          readonly kind: "solid";
          readonly color: SnapshotColor;
          readonly opacity: number;
      }
    | {
          readonly kind: "linear-gradient";
          readonly transform: SnapshotTransform;
          readonly stops: readonly {
              readonly position: number;
              readonly color: SnapshotColor;
          }[];
          readonly opacity: number;
      };

export type SnapshotImage = {
    readonly kind: "image";
    readonly mimeType: "image/png" | "image/jpeg" | "image/gif";
    readonly data: string;
    readonly intrinsicWidth: number;
    readonly intrinsicHeight: number;
    readonly scaleMode: "FILL" | "FIT" | "CROP" | "TILE";
    readonly tileScale?: number;
    readonly grayscale?: boolean;
    readonly crop?: readonly [number, number, number, number];
    readonly opacity: number;
};

export type SnapshotPaintWithImage = SnapshotPaint | SnapshotImage;

export type SnapshotStroke = {
    readonly paint: SnapshotPaint;
    /** Omitted means Figma's inside alignment. */
    readonly align?: "INSIDE" | "center" | "outside";
    readonly strokeTopWeight: number;
    readonly strokeRightWeight: number;
    readonly strokeBottomWeight: number;
    readonly strokeLeftWeight: number;
    /** The original dash pattern. A non-empty pattern is rendered solid. */
    readonly dashPattern: readonly number[];
};

export type SnapshotShadow = {
    readonly kind?: "inner";
    readonly color: SnapshotColor;
    readonly offsetX: number;
    readonly offsetY: number;
    readonly blur: number;
    readonly spread: number;
};

export type SnapshotAppearance = {
    readonly fills: readonly SnapshotPaintWithImage[];
    readonly strokes: readonly SnapshotStroke[];
    readonly cornerRadii: readonly [number, number, number, number];
    readonly clipsContent: boolean;
    readonly shadows: readonly SnapshotShadow[];
};

type SnapshotGeometry = {
    readonly x: number;
    readonly y: number;
    readonly width: number;
    readonly height: number;
    readonly opacity: number;
    readonly visible: boolean;
    readonly rotation: number;
    readonly layoutPositioning: "auto" | "absolute";
};

/**
 * A PNG exported for one visual node. The pixel dimensions are recorded next
 * to the bytes because the export density is part of the visual provenance;
 * consumers map this raster to its painted bounds when present, otherwise to the node's logical bounds.
 */
export type SnapshotRaster = {
    readonly bounds?: VisualBounds;
    readonly data: string;
    readonly exportScale: number;
    readonly pixelWidth: number;
    readonly pixelHeight: number;
};

type SnapshotSizingMetadata = {
    readonly layoutSizingHorizontal: SnapshotSizing | null;
    readonly layoutSizingVertical: SnapshotSizing | null;
};

type SnapshotContainerMetadata = {
    /** A renderer-level description, never a Figma node type. */
    readonly containerKind: "group" | "native" | "section" | "generic";
    readonly layoutFallback: "flex" | "freeform";
};

type SnapshotGroupNode = SnapshotGeometry &
    SnapshotSizingMetadata &
    SnapshotContainerMetadata & {
        readonly kind: "group";
        readonly id: string;
        readonly name: string;
        readonly children: readonly SnapshotNode[];
    };

type SnapshotFrameContainer = SnapshotGeometry &
    SnapshotSizingMetadata &
    SnapshotContainerMetadata &
    SnapshotAppearance & {
        readonly id: string;
        readonly name: string;
        readonly autoLayout: SnapshotAutoLayout | null;
        readonly children: readonly SnapshotNode[];
    };

type SnapshotFrameNode = SnapshotFrameContainer & {
    readonly kind: "frame";
};

type SnapshotComponentNode = SnapshotFrameContainer & {
    readonly kind: "component";
};

type SnapshotInstanceNode = SnapshotFrameContainer & {
    readonly kind: "instance";
};

type SnapshotComponentSetNode = SnapshotFrameContainer & {
    readonly kind: "component-set";
};

type SnapshotSectionNode = SnapshotFrameContainer & {
    readonly kind: "section";
};

type SnapshotContainerNode = SnapshotFrameContainer & {
    readonly kind: "container";
};

export type SnapshotFrameLikeNode =
    | SnapshotFrameNode
    | SnapshotComponentNode
    | SnapshotInstanceNode
    | SnapshotComponentSetNode
    | SnapshotSectionNode
    | SnapshotContainerNode;

export type SnapshotRectangleNode = SnapshotGeometry &
    SnapshotSizingMetadata &
    SnapshotAppearance & {
        readonly kind: "rectangle";
        readonly id: string;
        readonly name: string;
    };

type SnapshotSvgNode = SnapshotGeometry &
    SnapshotSizingMetadata & {
        readonly kind: "svg";
        readonly id: string;
        readonly name: string;
        /** Source node type is metadata, not a converter dependency on Figma. */
        readonly sourceType: string;
        /** SVG provenance when Figma's SVG exporter accepts the node. */
        readonly svg?: string;
        /** Figma's raster export, retained with its density and bounds. */
        readonly raster?: SnapshotRaster;
        readonly paintBounds?: VisualBounds;
    };

export type SnapshotTextRun = {
    readonly range: readonly [number, number];
    readonly text: string;
    readonly color: SnapshotColor | null;
    readonly bold: boolean;
    readonly italic: boolean;
    readonly underline: boolean;
    readonly strike: boolean;
};

export type SnapshotTextAutoResize =
    | "none"
    | "width-and-height"
    | "height"
    | "truncate";

export type SnapshotTextNode = SnapshotGeometry & {
    readonly layoutSizingHorizontal: SnapshotSizing | null;
    readonly layoutSizingVertical: SnapshotSizing | null;
    readonly kind: "text";
    readonly id: string;
    readonly name: string;
    readonly characters: string;
    readonly fills: readonly SnapshotPaintWithImage[];
    readonly fontFamily: string;
    readonly fontStyle: string;
    readonly fontSize: number;
    readonly fontWeight: number;
    readonly textAutoResize: SnapshotTextAutoResize;
    readonly horizontalAlign: "LEFT" | "CENTER" | "RIGHT" | "JUSTIFIED";
    readonly verticalAlign: "TOP" | "CENTER" | "BOTTOM";
    readonly italic: boolean;
    readonly letterSpacing: number;
    readonly lineHeightFactor: number | null;
    readonly wrap: boolean;
    readonly overflow: "clip" | "elide";
    readonly maxLines: number | null;
    readonly runs: readonly SnapshotTextRun[];
};

export type SnapshotNode =
    | SnapshotGroupNode
    | SnapshotFrameNode
    | SnapshotComponentNode
    | SnapshotInstanceNode
    | SnapshotComponentSetNode
    | SnapshotSectionNode
    | SnapshotContainerNode
    | SnapshotRectangleNode
    | SnapshotSvgNode
    | SnapshotTextNode;

export type FigmaSnapshot = {
    readonly schemaVersion: typeof SNAPSHOT_SCHEMA_VERSION;
    readonly selection: {
        readonly nodeId: string;
        readonly nodeName: string;
    };
    readonly root: SnapshotNode;
    readonly components?: ComponentLibrary<SnapshotNode>;
    readonly variables?: VariableLibrary;
};

export type Diagnostic = {
    readonly severity: "error" | "warning";
    readonly code: string;
    readonly nodeId?: string;
    readonly nodeName?: string;
    readonly propertyPath?: string;
    readonly nodePath?: string;
    readonly originalValue?: string;
    readonly fallbackAction?: string;
    readonly category?: "approximation" | "image" | "omission" | "geometry";
    readonly message: string;
};

export type CaptureWork = {
    readonly capturedNodes: number;
    readonly componentFamilies: number;
    readonly componentVariants: number;
    readonly pngExports: number;
    readonly svgExports: number;
    readonly textCacheHits: number;
};

export type CaptureMetrics = {
    readonly durationMs: number;
    readonly requests: number;
    readonly exports: number;
    readonly cacheHits: number;
    readonly work?: CaptureWork;
};

export type CaptureResult =
    | {
          readonly ok: true;
          readonly empty?: false;
          readonly snapshot: FigmaSnapshot;
          readonly nodeIds: readonly string[];
          readonly warnings: readonly Diagnostic[];
          readonly captureMetrics: CaptureMetrics;
      }
    | {
          readonly ok: true;
          readonly empty: true;
          readonly nodeIds: readonly [];
          readonly captureMetrics: CaptureMetrics;
      }
    | {
          readonly ok: false;
          readonly diagnostics: readonly Diagnostic[];
          readonly nodeIds: readonly string[];
          readonly captureMetrics: CaptureMetrics;
      };

export type SnapshotValidationResult =
    | { readonly ok: true; readonly snapshot: FigmaSnapshot }
    | { readonly ok: false; readonly diagnostics: readonly Diagnostic[] };

const diagnostic = (
    message: string,
    propertyPath?: string,
    code = "INVALID_SNAPSHOT",
): Diagnostic => ({
    severity: "error",
    code,
    propertyPath,
    message,
});

function recordObject(value: unknown): value is Record<string, unknown> {
    return typeof value === "object" && value !== null && !Array.isArray(value);
}

function finite(value: unknown): value is number {
    return typeof value === "number" && Number.isFinite(value);
}

function nonEmptyString(value: unknown): value is string {
    return typeof value === "string" && value.length > 0;
}

const base64Alphabet =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

function decodeBase64(value: string): Uint8Array | undefined {
    if (
        value.length === 0 ||
        value.length % 4 !== 0 ||
        !/^[A-Za-z0-9+/]*={0,2}$/.test(value)
    )
        return undefined;
    const padding = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
    const decoded = new Uint8Array((value.length / 4) * 3 - padding);
    let offset = 0;
    for (let index = 0; index < value.length; index += 4) {
        const first = base64Alphabet.indexOf(value.charAt(index));
        const second = base64Alphabet.indexOf(value.charAt(index + 1));
        const third =
            value.charAt(index + 2) === "="
                ? 0
                : base64Alphabet.indexOf(value.charAt(index + 2));
        const fourth =
            value.charAt(index + 3) === "="
                ? 0
                : base64Alphabet.indexOf(value.charAt(index + 3));
        if (first < 0 || second < 0 || third < 0 || fourth < 0)
            return undefined;
        if (offset < decoded.length)
            decoded[offset++] = (first << 2) | (second >> 4);
        if (value[index + 2] !== "=" && offset < decoded.length)
            decoded[offset++] = ((second & 0x0f) << 4) | (third >> 2);
        if (value[index + 3] !== "=" && offset < decoded.length)
            decoded[offset++] = ((third & 0x03) << 6) | fourth;
    }
    return decoded;
}

function pngDimensions(
    value: string,
): { readonly width: number; readonly height: number } | undefined {
    const bytes = decodeBase64(value);
    if (
        bytes === undefined ||
        bytes.length < 33 ||
        ![137, 80, 78, 71, 13, 10, 26, 10].every(
            (byte, index) => bytes[index] === byte,
        ) ||
        bytes[8] !== 0 ||
        bytes[9] !== 0 ||
        bytes[10] !== 0 ||
        bytes[11] !== 13 ||
        bytes[12] !== 73 ||
        bytes[13] !== 72 ||
        bytes[14] !== 68 ||
        bytes[15] !== 82
    )
        return undefined;
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const width = view.getUint32(16);
    const height = view.getUint32(20);
    return width > 0 && height > 0 ? { width, height } : undefined;
}

function validateRaster(value: unknown, path: string): Diagnostic[] {
    if (!recordObject(value))
        return [diagnostic("Expected raster metadata", path)];
    const errors: Diagnostic[] = [];
    if (value.bounds !== undefined && !validVisualBounds(value.bounds))
        errors.push(
            diagnostic("Invalid raster painted bounds", `${path}.bounds`),
        );
    if (!nonEmptyString(value.data))
        errors.push(
            diagnostic(
                "Raster data must be a non-empty base64 string",
                `${path}.data`,
            ),
        );
    const dimensions =
        typeof value.data === "string" ? pngDimensions(value.data) : undefined;
    if (dimensions === undefined)
        errors.push(
            diagnostic(
                "Raster data must be a valid PNG with an IHDR header",
                `${path}.data`,
            ),
        );
    if (!finite(value.exportScale) || value.exportScale <= 0)
        errors.push(
            diagnostic(
                "Raster exportScale must be positive",
                `${path}.exportScale`,
            ),
        );
    for (const key of ["pixelWidth", "pixelHeight"] as const)
        if (
            typeof value[key] !== "number" ||
            !Number.isInteger(value[key]) ||
            value[key] <= 0
        )
            errors.push(
                diagnostic(
                    `Raster ${key} must be a positive integer`,
                    `${path}.${key}`,
                ),
            );
    if (
        dimensions !== undefined &&
        value.pixelWidth === dimensions.width &&
        value.pixelHeight === dimensions.height
    )
        return errors;
    if (dimensions !== undefined) {
        if (value.pixelWidth !== dimensions.width)
            errors.push(
                diagnostic(
                    `Raster pixelWidth must match PNG width ${dimensions.width}`,
                    `${path}.pixelWidth`,
                ),
            );
        if (value.pixelHeight !== dimensions.height)
            errors.push(
                diagnostic(
                    `Raster pixelHeight must match PNG height ${dimensions.height}`,
                    `${path}.pixelHeight`,
                ),
            );
    }
    return errors;
}

function validateColor(value: unknown, path: string): Diagnostic[] {
    if (!recordObject(value))
        return [diagnostic("Expected a color object", path)];
    return (["r", "g", "b", "a"] as const).flatMap((key) =>
        finite(value[key]) && value[key] >= 0 && value[key] <= 1
            ? []
            : [
                  diagnostic(
                      `Expected ${key} to be a finite value between 0 and 1`,
                      `${path}.${key}`,
                  ),
              ],
    );
}

function validateTransform(value: unknown, path: string): Diagnostic[] {
    if (!Array.isArray(value) || value.length !== 2)
        return [diagnostic("Expected a 2×3 transform matrix", path)];
    return value.flatMap((row, rowIndex) => {
        if (!Array.isArray(row) || row.length !== 3)
            return [
                diagnostic(
                    "Expected a 3-value transform row",
                    `${path}.${rowIndex}`,
                ),
            ];
        return row.every(finite)
            ? []
            : [
                  diagnostic(
                      "Transform values must be finite",
                      `${path}.${rowIndex}`,
                  ),
              ];
    });
}

function validatePaint(value: unknown, path: string): Diagnostic[] {
    if (
        !recordObject(value) ||
        (value.kind !== "solid" &&
            value.kind !== "linear-gradient" &&
            value.kind !== "image")
    )
        return [diagnostic("Expected a supported paint", path)];
    const errors =
        value.kind === "solid"
            ? validateColor(value.color, `${path}.color`)
            : [];
    if (!finite(value.opacity) || value.opacity < 0 || value.opacity > 1)
        errors.push(
            diagnostic(
                "Paint opacity must be between 0 and 1",
                `${path}.opacity`,
            ),
        );
    if (value.kind === "linear-gradient") {
        errors.push(...validateTransform(value.transform, `${path}.transform`));
        if (!Array.isArray(value.stops) || value.stops.length < 2)
            errors.push(
                diagnostic(
                    "A gradient requires at least two stops",
                    `${path}.stops`,
                ),
            );
        else
            value.stops.forEach((stop, index) => {
                if (
                    !recordObject(stop) ||
                    !finite(stop.position) ||
                    stop.position < 0 ||
                    stop.position > 1
                )
                    errors.push(
                        diagnostic(
                            "Gradient stop position must be between 0 and 1",
                            `${path}.stops.${index}.position`,
                        ),
                    );
                errors.push(
                    ...validateColor(
                        recordObject(stop) ? stop.color : undefined,
                        `${path}.stops.${index}.color`,
                    ),
                );
            });
    }
    if (value.kind === "image") {
        if (
            value.mimeType !== "image/png" &&
            value.mimeType !== "image/jpeg" &&
            value.mimeType !== "image/gif"
        )
            errors.push(
                diagnostic("Unsupported image MIME type", `${path}.mimeType`),
            );
        if (
            typeof value.data !== "string" ||
            value.data.length === 0 ||
            !/^[A-Za-z0-9+/]*={0,2}$/.test(value.data) ||
            value.data.length % 4 !== 0
        )
            errors.push(
                diagnostic(
                    "Image data must be a non-empty base64 string",
                    `${path}.data`,
                ),
            );
        for (const key of ["intrinsicWidth", "intrinsicHeight"] as const)
            if (!finite(value[key]) || value[key] <= 0)
                errors.push(
                    diagnostic(`${key} must be positive`, `${path}.${key}`),
                );
        if (!["FILL", "FIT", "CROP", "TILE"].includes(String(value.scaleMode)))
            errors.push(
                diagnostic("Unsupported image scale mode", `${path}.scaleMode`),
            );
        if (
            value.tileScale !== undefined &&
            (!finite(value.tileScale) || value.tileScale <= 0)
        )
            errors.push(
                diagnostic(
                    "Image tile scale must be positive",
                    `${path}.tileScale`,
                ),
            );
        if (
            value.grayscale !== undefined &&
            typeof value.grayscale !== "boolean"
        )
            errors.push(
                diagnostic(
                    "Image grayscale must be boolean",
                    `${path}.grayscale`,
                ),
            );
        if (value.crop !== undefined) {
            if (
                !Array.isArray(value.crop) ||
                value.crop.length !== 4 ||
                value.crop.some((entry) => !finite(entry))
            )
                errors.push(
                    diagnostic(
                        "Image crop must be four finite values",
                        `${path}.crop`,
                    ),
                );
            else {
                const [x, y, width, height] = value.crop;
                if (
                    x < 0 ||
                    y < 0 ||
                    width <= 0 ||
                    height <= 0 ||
                    x + width > 1 ||
                    y + height > 1
                )
                    errors.push(
                        diagnostic(
                            "Image crop must be inside the normalized image bounds",
                            `${path}.crop`,
                        ),
                    );
            }
        }
    }
    return errors;
}

function validateGeometry(
    value: Record<string, unknown>,
    path: string,
): Diagnostic[] {
    const errors: Diagnostic[] = [];
    for (const key of [
        "x",
        "y",
        "width",
        "height",
        "opacity",
        "rotation",
    ] as const)
        if (!finite(value[key]))
            errors.push(diagnostic(`${key} must be finite`, `${path}.${key}`));
    if (finite(value.width) && value.width < 0)
        errors.push(diagnostic("Width cannot be negative", `${path}.width`));
    if (finite(value.height) && value.height < 0)
        errors.push(diagnostic("Height cannot be negative", `${path}.height`));
    if (!finite(value.opacity) || value.opacity < 0 || value.opacity > 1)
        errors.push(
            diagnostic("Opacity must be between 0 and 1", `${path}.opacity`),
        );
    if (typeof value.visible !== "boolean")
        errors.push(diagnostic("Visible must be a boolean", `${path}.visible`));
    if (
        value.layoutPositioning !== "auto" &&
        value.layoutPositioning !== "absolute"
    )
        errors.push(
            diagnostic(
                "layoutPositioning must be auto or absolute",
                `${path}.layoutPositioning`,
            ),
        );
    return errors;
}

function validateSizing(
    value: Record<string, unknown>,
    path: string,
): Diagnostic[] {
    const errors: Diagnostic[] = [];
    for (const key of [
        "layoutSizingHorizontal",
        "layoutSizingVertical",
    ] as const)
        if (
            value[key] !== null &&
            value[key] !== "fixed" &&
            value[key] !== "hug" &&
            value[key] !== "fill"
        )
            errors.push(
                diagnostic(
                    `${key} must be fixed, hug, fill, or null`,
                    `${path}.${key}`,
                ),
            );
    return errors;
}

function validateAutoLayout(value: unknown, path: string): Diagnostic[] {
    if (!recordObject(value))
        return [diagnostic("Auto-layout metadata must be an object", path)];
    const errors: Diagnostic[] = [];
    if (value.direction !== "horizontal" && value.direction !== "vertical")
        errors.push(
            diagnostic(
                "Auto-layout direction must be horizontal or vertical",
                `${path}.direction`,
            ),
        );
    for (const key of [
        "paddingLeft",
        "paddingRight",
        "paddingTop",
        "paddingBottom",
    ] as const)
        if (!finite(value[key]) || value[key] < 0)
            errors.push(
                diagnostic(
                    `${key} must be a non-negative finite number`,
                    `${path}.${key}`,
                ),
            );
    if (
        !["start", "center", "end", "space-between"].includes(
            String(value.primaryAlignment),
        )
    )
        errors.push(
            diagnostic(
                "Unsupported primary auto-layout alignment",
                `${path}.primaryAlignment`,
            ),
        );
    if (!["start", "center", "end"].includes(String(value.counterAlignment)))
        errors.push(
            diagnostic(
                "Unsupported counter auto-layout alignment",
                `${path}.counterAlignment`,
            ),
        );
    if (typeof value.wrap !== "boolean")
        errors.push(
            diagnostic("Auto-layout wrap must be a boolean", `${path}.wrap`),
        );
    if (
        value.reversePaintOrder !== undefined &&
        value.reversePaintOrder !== true
    )
        errors.push(
            diagnostic(
                "reversePaintOrder must be true when present",
                `${path}.reversePaintOrder`,
            ),
        );
    for (const key of ["itemSpacing", "counterAxisSpacing"] as const)
        if (!finite(value[key]))
            errors.push(diagnostic(`${key} must be finite`, `${path}.${key}`));
    if (
        !["start", "center", "end", "space-between", "stretch"].includes(
            String(value.counterAxisAlignContent),
        )
    )
        errors.push(
            diagnostic(
                "Unsupported counter-axis line alignment",
                `${path}.counterAxisAlignContent`,
            ),
        );
    return errors;
}

function validateAppearance(
    value: Record<string, unknown>,
    path: string,
): Diagnostic[] {
    const errors: Diagnostic[] = [];
    if (!Array.isArray(value.fills))
        errors.push(diagnostic("Fills must be an array", `${path}.fills`));
    else
        value.fills.forEach((paint, index) => {
            errors.push(...validatePaint(paint, `${path}.fills.${index}`));
        });
    if (!Array.isArray(value.strokes))
        errors.push(diagnostic("Strokes must be an array", `${path}.strokes`));
    else
        value.strokes.forEach((stroke, index) => {
            if (!recordObject(stroke)) {
                errors.push(
                    diagnostic(
                        "Expected a stroke object",
                        `${path}.strokes.${index}`,
                    ),
                );
                return;
            }
            errors.push(
                ...validatePaint(
                    stroke.paint,
                    `${path}.strokes.${index}.paint`,
                ),
            );
            if (
                stroke.align !== undefined &&
                stroke.align !== "INSIDE" &&
                stroke.align !== "center" &&
                stroke.align !== "outside"
            )
                errors.push(
                    diagnostic(
                        "Stroke alignment must be center or outside",
                        `${path}.strokes.${index}.align`,
                    ),
                );
            for (const key of [
                "strokeTopWeight",
                "strokeRightWeight",
                "strokeBottomWeight",
                "strokeLeftWeight",
            ] as const)
                if (!finite(stroke[key]) || stroke[key] < 0)
                    errors.push(
                        diagnostic(
                            "Stroke weight must be non-negative",
                            `${path}.strokes.${index}.${key}`,
                        ),
                    );
            if (
                !Array.isArray(stroke.dashPattern) ||
                stroke.dashPattern.some((entry) => !finite(entry) || entry < 0)
            )
                errors.push(
                    diagnostic(
                        "Dash pattern must contain non-negative finite values",
                        `${path}.strokes.${index}.dashPattern`,
                    ),
                );
        });
    if (!Array.isArray(value.cornerRadii) || value.cornerRadii.length !== 4)
        errors.push(
            diagnostic(
                "cornerRadii must contain four values",
                `${path}.cornerRadii`,
            ),
        );
    else
        value.cornerRadii.forEach((radius, index) => {
            if (!finite(radius) || radius < 0)
                errors.push(
                    diagnostic(
                        "Corner radius must be non-negative",
                        `${path}.cornerRadii.${index}`,
                    ),
                );
        });
    if (typeof value.clipsContent !== "boolean")
        errors.push(
            diagnostic(
                "clipsContent must be a boolean",
                `${path}.clipsContent`,
            ),
        );
    if (!Array.isArray(value.shadows))
        errors.push(diagnostic("shadows must be an array", `${path}.shadows`));
    else
        value.shadows.forEach((shadow, index) => {
            const shadowPath = `${path}.shadows.${index}`;
            if (!recordObject(shadow)) {
                errors.push(diagnostic("Expected a shadow object", shadowPath));
                return;
            }
            errors.push(...validateColor(shadow.color, `${shadowPath}.color`));
            if (shadow.kind !== undefined && shadow.kind !== "inner")
                errors.push(
                    diagnostic("Unsupported shadow kind", `${shadowPath}.kind`),
                );
            for (const key of ["offsetX", "offsetY", "blur", "spread"] as const)
                if (!finite(shadow[key]) || (key === "blur" && shadow[key] < 0))
                    errors.push(
                        diagnostic(
                            `${key} must be finite${key === "blur" ? " and non-negative" : ""}`,
                            `${shadowPath}.${key}`,
                        ),
                    );
        });
    return errors;
}

function validateContainerMetadata(
    value: Record<string, unknown>,
    path: string,
): Diagnostic[] {
    const errors: Diagnostic[] = [];
    if (
        !["group", "native", "section", "generic"].includes(
            String(value.containerKind),
        )
    )
        errors.push(
            diagnostic("Unsupported container kind", `${path}.containerKind`),
        );
    if (value.layoutFallback !== "flex" && value.layoutFallback !== "freeform")
        errors.push(
            diagnostic(
                "layoutFallback must be flex or freeform",
                `${path}.layoutFallback`,
            ),
        );
    return errors;
}

function validateTextRuns(
    value: Record<string, unknown>,
    path: string,
): Diagnostic[] {
    if (!Array.isArray(value.runs))
        return [diagnostic("Text runs must be an array", `${path}.runs`)];
    const characters =
        typeof value.characters === "string" ? value.characters : "";
    const errors: Diagnostic[] = [];
    let previousEnd = 0;
    value.runs.forEach((run, index) => {
        const runPath = `${path}.runs.${index}`;
        if (!recordObject(run)) {
            errors.push(diagnostic("Expected a text run object", runPath));
            return;
        }
        if (
            !Array.isArray(run.range) ||
            run.range.length !== 2 ||
            !run.range.every((entry) => Number.isInteger(entry))
        ) {
            errors.push(
                diagnostic(
                    "Text run range must contain two integer offsets",
                    `${runPath}.range`,
                ),
            );
        } else {
            const [start, end] = run.range;
            if (
                start < 0 ||
                end < start ||
                end > characters.length ||
                start < previousEnd
            )
                errors.push(
                    diagnostic(
                        "Text run range must be ordered and inside characters",
                        `${runPath}.range`,
                    ),
                );
            else previousEnd = end;
            if (
                typeof run.text !== "string" ||
                run.text !== characters.slice(start, end)
            )
                errors.push(
                    diagnostic(
                        "Text run text must match its character range",
                        `${runPath}.text`,
                    ),
                );
        }
        for (const key of ["bold", "italic", "underline", "strike"] as const)
            if (typeof run[key] !== "boolean")
                errors.push(
                    diagnostic(`${key} must be a boolean`, `${runPath}.${key}`),
                );
        if (run.color !== null)
            errors.push(...validateColor(run.color, `${runPath}.color`));
    });
    return errors;
}

function validateNode(value: unknown, path: string): Diagnostic[] {
    if (!recordObject(value))
        return [diagnostic("Expected a node object", path)];
    const errors = [
        ...validateGeometry(value, path),
        ...validateSizing(value, path),
    ];
    if (!nonEmptyString(value.id))
        errors.push(diagnostic("Node id is required", `${path}.id`));
    if (typeof value.name !== "string")
        errors.push(diagnostic("Node name must be a string", `${path}.name`));
    const containerKind =
        value.kind === "group" ||
        value.kind === "frame" ||
        value.kind === "component" ||
        value.kind === "instance" ||
        value.kind === "component-set" ||
        value.kind === "section" ||
        value.kind === "container";
    if (containerKind) errors.push(...validateContainerMetadata(value, path));
    if (value.kind === "group") {
        if (!Array.isArray(value.children))
            errors.push(
                diagnostic(
                    "Container children must be an array",
                    `${path}.children`,
                ),
            );
        else
            value.children.forEach((child, index) => {
                errors.push(
                    ...validateNode(child, `${path}.children.${index}`),
                );
            });
    } else if (
        value.kind === "frame" ||
        value.kind === "component" ||
        value.kind === "instance" ||
        value.kind === "component-set" ||
        value.kind === "section" ||
        value.kind === "container"
    ) {
        if (value.autoLayout !== null && value.autoLayout === undefined)
            errors.push(
                diagnostic(
                    "Frame auto-layout metadata is required",
                    `${path}.autoLayout`,
                ),
            );
        else if (value.autoLayout !== null)
            errors.push(
                ...validateAutoLayout(value.autoLayout, `${path}.autoLayout`),
            );
        if (!Array.isArray(value.children))
            errors.push(
                diagnostic(
                    "Frame children must be an array",
                    `${path}.children`,
                ),
            );
        else
            value.children.forEach((child, index) => {
                errors.push(
                    ...validateNode(child, `${path}.children.${index}`),
                );
            });
        errors.push(...validateAppearance(value, path));
    } else if (value.kind === "rectangle") {
        errors.push(...validateAppearance(value, path));
    } else if (value.kind === "svg") {
        if (value.png !== undefined)
            errors.push(
                diagnostic(
                    "Legacy SVG raster field png is unsupported; use raster metadata",
                    `${path}.png`,
                ),
            );
        if (
            !nonEmptyString(value.sourceType) ||
            ![
                "VECTOR",
                "BOOLEAN_OPERATION",
                "ELLIPSE",
                "LINE",
                "POLYGON",
                "STAR",
                "CONNECTOR",
                "SHAPE_WITH_TEXT",
                "TEXT",
                "MASK_COMPOSITION",
            ].includes(value.sourceType)
        )
            errors.push(
                diagnostic("Unsupported SVG source type", `${path}.sourceType`),
            );
        if (value.svg !== undefined) {
            if (typeof value.svg !== "string" || value.svg.trim().length === 0)
                errors.push(
                    diagnostic(
                        "SVG content must be a non-empty string",
                        `${path}.svg`,
                    ),
                );
            else if (
                !value.svg
                    .replaceAll("\r\n", "\n")
                    .replaceAll("\r", "\n")
                    .trim()
                    .startsWith("<svg")
            )
                errors.push(
                    diagnostic(
                        "SVG content must begin with an <svg document",
                        `${path}.svg`,
                    ),
                );
        }
        if (
            value.paintBounds !== undefined &&
            !validVisualBounds(value.paintBounds)
        )
            errors.push(
                diagnostic("Invalid SVG painted bounds", `${path}.paintBounds`),
            );
        if (value.raster !== undefined)
            errors.push(...validateRaster(value.raster, `${path}.raster`));
        if (value.svg === undefined && value.raster === undefined)
            errors.push(
                diagnostic(
                    "SVG nodes must contain SVG content or Figma raster pixels",
                    path,
                ),
            );
    } else if (value.kind === "text") {
        if (typeof value.characters !== "string")
            errors.push(
                diagnostic(
                    "Text characters must be a string",
                    `${path}.characters`,
                ),
            );
        if (
            value.textAutoResize !== "none" &&
            value.textAutoResize !== "width-and-height" &&
            value.textAutoResize !== "height" &&
            value.textAutoResize !== "truncate"
        )
            errors.push(
                diagnostic(
                    "Text auto-resize must be none, width-and-height, height, or truncate",
                    `${path}.textAutoResize`,
                ),
            );
        if (!Array.isArray(value.fills))
            errors.push(
                diagnostic("Text fills must be an array", `${path}.fills`),
            );
        else
            value.fills.forEach((paint, index) => {
                errors.push(...validatePaint(paint, `${path}.fills.${index}`));
            });
        for (const key of ["fontFamily", "fontStyle"] as const)
            if (typeof value[key] !== "string")
                errors.push(
                    diagnostic(`${key} must be a string`, `${path}.${key}`),
                );
        if (typeof value.italic !== "boolean")
            errors.push(
                diagnostic("Text italic must be a boolean", `${path}.italic`),
            );
        if (!finite(value.letterSpacing))
            errors.push(
                diagnostic(
                    "Text letter spacing must be finite",
                    `${path}.letterSpacing`,
                ),
            );
        if (
            value.lineHeightFactor !== null &&
            (!finite(value.lineHeightFactor) || value.lineHeightFactor <= 0)
        )
            errors.push(
                diagnostic(
                    "Line height factor must be positive or null",
                    `${path}.lineHeightFactor`,
                ),
            );
        if (typeof value.wrap !== "boolean")
            errors.push(
                diagnostic("Text wrap must be a boolean", `${path}.wrap`),
            );
        if (value.overflow !== "clip" && value.overflow !== "elide")
            errors.push(
                diagnostic(
                    "Text overflow must be clip or elide",
                    `${path}.overflow`,
                ),
            );
        if (
            value.maxLines !== null &&
            (!Number.isInteger(value.maxLines) || Number(value.maxLines) < 1)
        )
            errors.push(
                diagnostic(
                    "maxLines must be a positive integer or null",
                    `${path}.maxLines`,
                ),
            );
        if (!finite(value.fontSize) || value.fontSize <= 0)
            errors.push(
                diagnostic("Font size must be positive", `${path}.fontSize`),
            );
        if (!finite(value.fontWeight))
            errors.push(
                diagnostic("Font weight must be finite", `${path}.fontWeight`),
            );
        if (
            !["LEFT", "CENTER", "RIGHT", "JUSTIFIED"].includes(
                String(value.horizontalAlign),
            )
        )
            errors.push(
                diagnostic(
                    "Unsupported horizontal alignment",
                    `${path}.horizontalAlign`,
                ),
            );
        if (!["TOP", "CENTER", "BOTTOM"].includes(String(value.verticalAlign)))
            errors.push(
                diagnostic(
                    "Unsupported vertical alignment",
                    `${path}.verticalAlign`,
                ),
            );
        errors.push(...validateTextRuns(value, path));
    } else
        errors.push(
            diagnostic(
                `Unsupported node kind ${String(value.kind)}`,
                `${path}.kind`,
            ),
        );
    return errors;
}

export function validateSnapshot(value: unknown): SnapshotValidationResult {
    if (!recordObject(value))
        return {
            ok: false,
            diagnostics: [diagnostic("Snapshot must be an object")],
        };
    const errors: Diagnostic[] = [];
    if (value.schemaVersion !== SNAPSHOT_SCHEMA_VERSION)
        errors.push(
            diagnostic(
                `Unsupported snapshot schema version ${String(value.schemaVersion)}`,
                "schemaVersion",
            ),
        );
    if (!recordObject(value.selection))
        errors.push(diagnostic("Snapshot selection is required", "selection"));
    else {
        if (!nonEmptyString(value.selection.nodeId))
            errors.push(
                diagnostic("Selection nodeId is required", "selection.nodeId"),
            );
        if (typeof value.selection.nodeName !== "string")
            errors.push(
                diagnostic(
                    "Selection nodeName must be a string",
                    "selection.nodeName",
                ),
            );
    }
    errors.push(...validateNode(value.root, "root"));
    if (value.variables !== undefined) {
        try {
            validateVariableLibrary(value.variables);
        } catch {
            errors.push(diagnostic("Invalid variable library", "variables"));
        }
    }
    if (value.components !== undefined) {
        try {
            validateComponentLibrary<SnapshotNode>(value.components, (node) => {
                errors.push(...validateNode(node, "components"));
            });
        } catch {
            errors.push(diagnostic("Invalid component library", "components"));
        }
    }
    return errors.length === 0
        ? { ok: true, snapshot: value as FigmaSnapshot }
        : { ok: false, diagnostics: errors };
}

export function parseSnapshot(json: string): SnapshotValidationResult {
    try {
        return validateSnapshot(JSON.parse(json) as unknown);
    } catch (error) {
        return {
            ok: false,
            diagnostics: [
                diagnostic(
                    `Snapshot JSON could not be parsed: ${error instanceof Error ? error.message : String(error)}`,
                ),
            ],
        };
    }
}

export function serializeSnapshot(snapshot: FigmaSnapshot): string {
    const validation = validateSnapshot(snapshot);
    if (!validation.ok)
        throw new Error(
            validation.diagnostics.map((item) => item.message).join("; "),
        );
    return JSON.stringify(snapshot);
}

// These messages embed node names or generated identifiers in the raw
// diagnostic. Document summaries describe the limitation across all nodes.
const genericWarningMessages: Readonly<Record<string, string>> = {
    SVG_EXPORT_FALLBACK:
        "Figma SVG export was unavailable; using Figma raster provenance",
    COMPONENT_PROPERTY_STATIC:
        "Some authored properties have no editable native binding in this capture (for example, rasterized text)",
    COMPONENT_SWAP_STATIC:
        "Instance swaps retain the captured appearance; a runtime component mapping is required",
    COMPONENT_VARIANT_DOMAIN:
        "Only captured variant combinations are supported; other values render empty",
    COMPONENT_INSTANCE_SPECIALIZED:
        "Overrides outside the authored contract use private component specializations",
    COMPONENT_TOKEN_STATIC:
        "Captured values are retained where local modes or values differ from the shared token context",
};

/** Summarize for documents without discarding the original per-node diagnostics. */
export function warningSummaries(
    warnings: readonly Diagnostic[],
): Pick<Diagnostic, "code" | "message" | "category">[] {
    const summaries = new Map<
        string,
        Pick<Diagnostic, "code" | "message" | "category">
    >();
    for (const warning of warnings) {
        const message = Object.hasOwn(genericWarningMessages, warning.code)
            ? genericWarningMessages[warning.code]
            : warning.message;
        const key = JSON.stringify([warning.code, message]);
        if (!summaries.has(key))
            summaries.set(key, {
                code: warning.code,
                message,
                category: warning.category,
            });
    }
    return [...summaries.values()];
}

// Preserve first occurrence and diagnostic order; serialize each identity once.
export function uniqueDiagnostics(
    diagnostics: readonly Diagnostic[],
    identity: (diagnostic: Diagnostic) => string = (item) =>
        JSON.stringify(item),
): Diagnostic[] {
    const seen = new Set<string>();
    return diagnostics.filter((item) => {
        const key = identity(item);
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
    });
}
