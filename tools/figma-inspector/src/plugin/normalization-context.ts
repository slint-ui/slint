// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { NormalizationAssets } from "../images";
import type {
    Diagnostic,
    SnapshotAutoLayout,
    SnapshotNode,
    SnapshotColor,
} from "./snapshot";
import type { VisualBounds } from "./source";
import type { GenerationTarget, ImageResolver } from "./normalize";

/** Decoded captured properties, never a live Figma node. This projection is
 * internal: property repair and strict normalization still validate values at
 * runtime. It intentionally exposes no host methods or parent references. */
export type MaterializedNode = {
    readonly id: string;
    readonly name: string;
    readonly type: string;
    readonly x: number;
    readonly y: number;
    readonly width: number;
    readonly height: number;
    readonly opacity: number;
    readonly visible: boolean;
    readonly rotation: number;
    readonly children?: readonly MaterializedNode[];
    readonly fills: readonly Paint[] | symbol;
    readonly strokes: readonly Paint[] | symbol;
    readonly effects?: readonly Effect[];
    readonly cornerRadius: unknown;
    readonly topLeftRadius: number;
    readonly topRightRadius: number;
    readonly bottomRightRadius: number;
    readonly bottomLeftRadius: number;
    readonly strokeWeight: unknown;
    readonly strokeTopWeight?: unknown;
    readonly strokeRightWeight?: unknown;
    readonly strokeBottomWeight?: unknown;
    readonly strokeLeftWeight?: unknown;
    readonly strokeAlign?: string;
    readonly dashPattern?: readonly number[];
    readonly clipsContent: boolean;
    readonly blendMode?: string;
    readonly isMask?: boolean;
    readonly scaleFactor?: unknown;
    readonly layoutMode: string;
    readonly layoutSizingHorizontal: unknown;
    readonly layoutSizingVertical: unknown;
    readonly layoutPositioning?: string;
    readonly layoutAlign?: string;
    readonly primaryAxisAlignItems: string;
    readonly counterAxisAlignItems: string;
    readonly counterAxisAlignContent?: string;
    readonly counterAxisSpacing?: number;
    readonly paddingLeft: number;
    readonly paddingRight: number;
    readonly paddingTop: number;
    readonly paddingBottom: number;
    readonly itemSpacing: number;
    readonly layoutWrap: string;
    readonly itemReverseZIndex?: boolean;
    readonly strokesIncludedInLayout?: boolean;
    readonly characters: string;
    readonly fontName: unknown;
    readonly fontSize: unknown;
    readonly fontWeight: unknown;
    readonly textCase?: unknown;
    readonly letterSpacing?: unknown;
    readonly lineHeight?: unknown;
    readonly textAutoResize?: unknown;
    readonly textTruncation?: string | symbol;
    readonly maxLines: number | null;
    readonly textAlignHorizontal: "LEFT" | "CENTER" | "RIGHT" | "JUSTIFIED";
    readonly textAlignVertical: "TOP" | "CENTER" | "BOTTOM";
    readonly sourceSegments?:
        | { readonly segments: readonly unknown[] }
        | { readonly error: unknown };
    readonly numRows?: unknown;
    readonly numColumns?: unknown;
    readonly rowIndex?: unknown;
    readonly columnIndex?: unknown;
    readonly text?: unknown;
    readonly sourceCells?: readonly unknown[];
    readonly sourceSvgOmitted?: boolean;
    readonly sourceSvgBounds?: VisualBounds;
    readonly sourceRasterBounds?: VisualBounds;
    readonly sourceMaskRaster?: boolean;
};

export type NormalizationContext = {
    readonly assets: NormalizationAssets;
    readonly target: GenerationTarget;
    readonly metrics: {
        durationMs: number;
        requests: number;
        exports: number;
        cacheHits: number;
    };
    readonly exportScale: number;
    readonly mixedValue: unknown;
    readonly imageResolver: ImageResolver;
    readonly exportSvgNode: (node: MaterializedNode) => Promise<string>;
    readonly exportPngNode?: (
        node: MaterializedNode,
        scale: number,
    ) => Promise<Uint8Array>;
};
export type TraversalOptions = {
    readonly parentLayout?: SnapshotAutoLayout;
    readonly isRoot?: boolean;
};
export type NodeResult = {
    node?: SnapshotNode;
    error?: Diagnostic;
    warnings?: Diagnostic[];
};

export type NormalizedGeometry = Pick<
    SnapshotNode,
    | "id"
    | "name"
    | "x"
    | "y"
    | "width"
    | "height"
    | "opacity"
    | "visible"
    | "rotation"
    | "layoutPositioning"
    | "layoutSizingHorizontal"
    | "layoutSizingVertical"
>;

export function problem(
    code: string,
    node: { id: string; name?: string } | undefined,
    message: string,
    propertyPath?: string,
): Diagnostic {
    return {
        severity: "error",
        code,
        nodeId: node?.id,
        nodeName: node && "name" in node ? node.name : undefined,
        propertyPath,
        message,
    };
}

export function isMixed(value: unknown, mixedValue: unknown): boolean {
    return value === mixedValue || typeof value === "symbol";
}

export function number(value: unknown): value is number {
    return typeof value === "number" && Number.isFinite(value);
}

export function color(
    value: RGB | RGBA,
    node: MaterializedNode,
): SnapshotColor {
    if (
        !number(value.r) ||
        !number(value.g) ||
        !number(value.b) ||
        value.r < 0 ||
        value.r > 1 ||
        value.g < 0 ||
        value.g > 1 ||
        value.b < 0 ||
        value.b > 1 ||
        ("a" in value && (!number(value.a) || value.a < 0 || value.a > 1))
    ) {
        throw new CompatibilityError(`Invalid color on ${node.name}`);
    }
    return {
        r: value.r,
        g: value.g,
        b: value.b,
        a: "a" in value && number(value.a) ? value.a : 1,
    };
}

export class CompatibilityError extends Error {}
