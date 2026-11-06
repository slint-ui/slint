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
import { radialGeometry } from "../gradient-geometry";

export type RenderContext = {
    helperIds?: Map<string, string>;
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

/** Keep generated stroke geometry in the original layer's paint coordinates. */
export function paintInBounds(
    value: SnapshotPaintWithImage,
    original: { readonly width: number; readonly height: number },
    bounds: {
        readonly x: number;
        readonly y: number;
        readonly width: number;
        readonly height: number;
    },
): string {
    if (
        value.kind === "solid" ||
        value.kind === "image" ||
        original.width <= 0 ||
        original.height <= 0
    )
        return paint(value);
    const [first, second] = value.transform;
    const row = ([a, c, e]: readonly number[]): [number, number, number] => [
        (a * bounds.width) / original.width,
        (c * bounds.height) / original.height,
        e + (a * bounds.x) / original.width + (c * bounds.y) / original.height,
    ];
    return paint({ ...value, transform: [row(first), row(second)] });
}

function gradientNumber(value: number): string {
    return String(Number(value.toPrecision(12)));
}

export function paint(value: SnapshotPaintWithImage): string {
    if (value.kind === "solid") return color(value.color, value.opacity);
    if (value.kind === "image" && value.grayscale) {
        const svg = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="${value.intrinsicWidth}" height="${value.intrinsicHeight}"><defs><filter id="grayscale" color-interpolation-filters="sRGB"><feColorMatrix type="saturate" values="0"/></filter></defs><image width="100%" height="100%" filter="url(#grayscale)" xlink:href="data:${value.mimeType};base64,${value.data}"/></svg>`;
        return `@image-url("data:image/svg+xml;base64,${utf8ToBase64(svg)}")`;
    }
    if (value.kind === "image")
        return `@image-url("data:${value.mimeType};base64,${value.data}")`;
    // Figma interpolates straight RGBA; Slint interpolates premultiplied RGBA.
    const resolvedStops = value.stops.flatMap((stop, index) => {
        const next = value.stops[index + 1];
        const subdivisions =
            next &&
            next.position > stop.position &&
            next.color.a !== stop.color.a &&
            ["r", "g", "b"].some(
                (channel) =>
                    next.color[channel as "r" | "g" | "b"] !==
                    stop.color[channel as "r" | "g" | "b"],
            )
                ? 16
                : 1;
        return Array.from({ length: subdivisions }, (_, sample) => {
            const fraction = sample / subdivisions;
            const mix = (a: number, b: number) => a + (b - a) * fraction;
            return sample === 0
                ? stop
                : {
                      position: mix(stop.position, next.position),
                      color: {
                          r: mix(stop.color.r, next.color.r),
                          g: mix(stop.color.g, next.color.g),
                          b: mix(stop.color.b, next.color.b),
                          a: mix(stop.color.a, next.color.a),
                      },
                  };
        });
    });
    if (value.kind === "radial-gradient") {
        const geometry = radialGeometry(value.transform);
        const stops = resolvedStops
            .map(
                (stop) =>
                    `${color(stop.color, value.opacity)} ${gradientNumber(stop.position * 100)}%`,
            )
            .join(", ");
        return `@radial-gradient(ellipse self.width * ${gradientNumber(geometry.radiusX)} self.height * ${gradientNumber(geometry.radiusY)} at self.width * ${gradientNumber(geometry.centerX)} self.height * ${gradientNumber(geometry.centerY)}, ${stops})`;
    }
    if (value.kind === "conic-gradient") {
        const geometry = radialGeometry(value.transform);
        const [, [b, d]] = value.transform;
        const angle =
            b === 0
                ? d > 0
                    ? "90deg"
                    : "270deg"
                : `90deg + Math.atan2(${gradientNumber(-b)} * self.height / 1px, ${gradientNumber(d)} * self.width / 1px)`;
        const stops = resolvedStops
            .map(
                (stop) =>
                    `${color(stop.color, value.opacity)} ${gradientNumber(stop.position * 360)}deg`,
            )
            .join(", ");
        return `@conic-gradient(from ${angle} at self.width * ${gradientNumber(geometry.centerX)} self.height * ${gradientNumber(geometry.centerY)}, ${stops})`;
    }
    const [a, c, offset] = value.transform[0];
    const extent = Math.abs(a) + Math.abs(c);
    // Figma's affine map gives the stop coordinate at each point in the layer.
    // Slint spans the whole box, so remap stops from that box's extrema.
    const start = offset + Math.min(0, a) + Math.min(0, c);
    const angle =
        a === 0 || c === 0
            ? `${gradientNumber((90 + (Math.atan2(c, a) * 180) / Math.PI + 360) % 360)}deg`
            : `90deg + Math.atan2(${gradientNumber(c)} * self.width / 1px, ${gradientNumber(a)} * self.height / 1px)`;
    const stops = resolvedStops
        .map(
            (stop) =>
                `${color(stop.color, value.opacity)} ${gradientNumber(((stop.position - start) / extent) * 100)}%`,
        )
        .join(", ");
    return `@linear-gradient(${angle}, ${stops})`;
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
