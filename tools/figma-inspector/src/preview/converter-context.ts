// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { binding, type SlintLine } from "./slint-ir";
import type { ComponentUses } from "./component-generator";
import type {
    Diagnostic,
    SnapshotAutoLayout,
    SnapshotColor,
    SnapshotNode,
    SnapshotPaintWithImage,
} from "../plugin/snapshot";
import { utf8ToBase64 } from "../images";

export type RenderContext = {
    readonly warnings: Diagnostic[];
    readonly componentUses?: ComponentUses;
    readonly componentTemplates: boolean;
    readonly preview: boolean;
};
export type NodePlacement = {
    readonly depth?: number;
    readonly normalizePosition?: boolean;
    readonly flexItem?: boolean;
    readonly parentLayout?: SnapshotAutoLayout;
    readonly parent?: SnapshotNode;
    readonly layoutChild?: boolean;
    readonly layoutOrder?: number;
    readonly definitionRoot?: boolean;
};

function round(value: number): number {
    return Number(value.toFixed(3));
}

export function number(value: number): string {
    return String(round(value));
}

function channel(value: number): string {
    return Math.max(0, Math.min(255, Math.round(value * 255)))
        .toString(16)
        .padStart(2, "0")
        .toUpperCase();
}

export function color(value: SnapshotColor, opacity = 1): string {
    const alpha = Math.max(0, Math.min(1, value.a * opacity));
    const base = `#${channel(value.r)}${channel(value.g)}${channel(value.b)}`;
    return alpha >= 1 ? base : `${base}${channel(alpha)}`;
}

export function paint(value: SnapshotPaintWithImage): string {
    if (value.kind === "solid") return color(value.color, value.opacity);
    if (value.kind === "image" && value.grayscale) {
        const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${value.intrinsicWidth}" height="${value.intrinsicHeight}"><defs><filter id="grayscale" color-interpolation-filters="sRGB"><feColorMatrix type="saturate" values="0"/></filter></defs><image width="100%" height="100%" filter="url(#grayscale)" xlink:href="data:${value.mimeType};base64,${value.data}"/></svg>`;
        return `@image-url("data:image/svg+xml;base64,${utf8ToBase64(svg)}")`;
    }
    if (value.kind === "image")
        return `@image-url("data:${value.mimeType};base64,${value.data}")`;
    const [a, b] = value.transform[0];
    const angle =
        (90 + Math.round(Math.atan2(b, a) * (180 / Math.PI)) + 360) % 360;
    const stops = value.stops
        .map(
            (stop) =>
                `${color(stop.color, value.opacity)} ${Math.round(stop.position * 100)}%`,
        )
        .join(", ");
    return `@linear-gradient(${angle}deg, ${stops})`;
}

export function escaped(value: string): string {
    return value
        .replaceAll("\\", "\\\\")
        .replaceAll('"', '\\"')
        .replaceAll("\r", "\\u{d}")
        .replaceAll("\n", "\\n");
}

/** Emit authored nonzero lengths in the caller's explicit property order. */
export function lengthProperties(
    values: readonly (readonly [string, number])[],
    depth: number,
): SlintLine[] {
    return values
        .filter(([, value]) => value !== 0)
        .map(([name, value]) => binding(name, `${number(value)}px`, depth));
}
