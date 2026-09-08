// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import {
    type VariableLibrary,
    validateVariableLibrary,
} from "./variable-library";
import { type ComponentLibrary, validateComponentLibrary } from "./components";
import { isPngByteArray, pngDimensions } from "../images";
export const SOURCE_MIXED = Symbol("captured-mixed");
export type SourceValue =
    | null
    | boolean
    | number
    | string
    | SourceValue[]
    | { [key: string]: SourceValue };
export type SourceResult<T> = { value?: T; error?: string };
export type VisualBounds = {
    x: number;
    y: number;
    width: number;
    height: number;
};
export function validVisualBounds(value: unknown): value is VisualBounds {
    if (!value || typeof value !== "object") return false;
    const b = value as VisualBounds;
    return (
        [b.x, b.y, b.width, b.height].every(Number.isFinite) &&
        b.width > 0 &&
        b.height > 0
    );
}
export type SourceBytes = number[] | Uint8Array;
export type SourceNode<Bytes extends SourceBytes = number[]> = {
    id: string;
    name: string;
    type: string;
    properties: { [key: string]: SourceValue };
    children?: SourceNode<Bytes>[];
    segments?: SourceResult<SourceValue>;
    cells?: SourceValue[];
    errors: { property: string; message: string }[];
    exports?: {
        svg: SourceResult<string>;
        png?: SourceResult<Bytes>;
        rasterBounds?: VisualBounds;
        svgBounds?: VisualBounds;
        // A successful raster makes SVG unnecessary; this is not an export failure.
        svgOmitted?: "png";
    };
};
export type SourceCapture<Bytes extends SourceBytes = number[]> = {
    sourceVersion: 1;
    components?: ComponentLibrary<SourceNode<Bytes>>;
    variables?: VariableLibrary;
    root: SourceNode<Bytes>;
    exportScale: number;
    pngEnabled: boolean;
    images: Record<
        string,
        SourceResult<{
            bytes: Bytes;
            width?: number;
            height?: number;
            sizeError?: string;
        }>
    >;
};

/** Scan captured metadata synchronously; only fetching image bytes needs I/O. */
export function sourceImageHashes(value: unknown): Set<string> {
    const hashes = new Set<string>();
    function visit(value: unknown): void {
        if (!value || typeof value !== "object") return;
        if ("imageHash" in value && typeof value.imageHash === "string")
            hashes.add(value.imageHash);
        for (const child of Object.values(value)) visit(child);
    }
    visit(value);
    return hashes;
}
export function encodeValue(
    value: unknown,
    mixed: unknown = SOURCE_MIXED,
): SourceValue {
    if (typeof value === "symbol" || value === mixed)
        return { $source: "mixed" };
    if (value === undefined) return { $source: "unavailable" };
    if (typeof value === "number")
        return Number.isFinite(value) ? value : { $source: String(value) };
    if (
        value === null ||
        typeof value === "string" ||
        typeof value === "boolean"
    )
        return value;
    if (Array.isArray(value)) return value.map((v) => encodeValue(v, mixed));
    if (typeof value === "object")
        return Object.fromEntries(
            Object.entries(value)
                .filter(([, v]) => typeof v !== "function")
                .map(([k, v]) => [k, encodeValue(v, mixed)]),
        );
    return { $source: "unavailable" };
}
export function decodeValue(value: {
    [key: string]: SourceValue;
}): Record<string, unknown>;
export function decodeValue(value: SourceValue | undefined): unknown;
export function decodeValue(value: SourceValue | undefined): unknown {
    if (value === undefined) return undefined;
    if (Array.isArray(value)) return value.map(decodeValue);
    if (value && typeof value === "object") {
        if (value.$source === "mixed") return SOURCE_MIXED;
        if (value.$source === "unavailable") return undefined;
        if (value.$source === "NaN") return Number.NaN;
        if (value.$source === "Infinity") return Number.POSITIVE_INFINITY;
        if (value.$source === "-Infinity") return Number.NEGATIVE_INFINITY;
        return Object.fromEntries(
            Object.entries(value).map(([k, v]) => [k, decodeValue(v)]),
        );
    }
    return value;
}
export function validateSource(value: SourceCapture<SourceBytes>): void {
    if (
        !value ||
        value.sourceVersion !== 1 ||
        !Number.isFinite(value.exportScale) ||
        value.exportScale <= 0 ||
        typeof value.pngEnabled !== "boolean" ||
        !value.images
    )
        throw new Error("Invalid source capture contract");
    function json(value: unknown): boolean {
        if (
            value === null ||
            typeof value === "string" ||
            typeof value === "boolean"
        )
            return true;
        if (typeof value === "number") return Number.isFinite(value);
        if (Array.isArray(value)) return value.every(json);
        if (typeof value !== "object" || value === null) return false;
        const object = value as Record<string, unknown>;
        if (
            "$source" in object &&
            !["mixed", "unavailable", "NaN", "Infinity", "-Infinity"].includes(
                String(object.$source),
            )
        )
            return false;
        return Object.values(object).every(json);
    }
    const bytes = (value: unknown): boolean =>
        isPngByteArray(value) ||
        (Array.isArray(value) &&
            value.every(
                (byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255,
            ));
    function result(
        value: SourceResult<unknown> | undefined,
        valid: (value: unknown) => boolean,
    ): boolean {
        return (
            value === undefined ||
            (typeof value === "object" &&
                value !== null &&
                (value.error === undefined ||
                    typeof value.error === "string") &&
                (value.value === undefined || valid(value.value)))
        );
    }
    for (const image of Object.values(value.images))
        if (
            !result(image, (item) => {
                if (!item || typeof item !== "object") return false;
                const data = item as {
                    bytes: unknown;
                    width?: number;
                    height?: number;
                    sizeError?: string;
                };
                return (
                    bytes(data.bytes) &&
                    (data.sizeError === undefined ||
                        typeof data.sizeError === "string") &&
                    ((data.width === undefined && data.height === undefined) ||
                        (typeof data.width === "number" &&
                            Number.isFinite(data.width) &&
                            data.width > 0 &&
                            typeof data.height === "number" &&
                            Number.isFinite(data.height) &&
                            data.height > 0))
                );
            })
        )
            throw new Error("Invalid source image contract");
    const ids = new Set<string>();
    function visit(node: SourceNode<SourceBytes>) {
        if (
            !node ||
            typeof node.id !== "string" ||
            typeof node.name !== "string" ||
            typeof node.type !== "string" ||
            !node.id ||
            !node.type ||
            !node.properties ||
            Array.isArray(node.properties) ||
            !json(node.properties) ||
            !result(node.segments, json) ||
            (node.cells !== undefined &&
                (!Array.isArray(node.cells) || !json(node.cells))) ||
            (node.exports !== undefined &&
                (!result(node.exports.svg, (v) => typeof v === "string") ||
                    !result(node.exports.png, bytes) ||
                    (node.exports.svgBounds !== undefined &&
                        !validVisualBounds(node.exports.svgBounds)) ||
                    (node.exports.rasterBounds !== undefined &&
                        !validVisualBounds(node.exports.rasterBounds)) ||
                    (node.exports.svgOmitted !== undefined &&
                        (node.exports.svgOmitted !== "png" ||
                            !value.pngEnabled ||
                            !node.exports.svg ||
                            node.exports.svg.value !== undefined ||
                            node.exports.svg.error !== undefined ||
                            (node.exports.png?.error !== undefined &&
                                node.exports.png.value !== undefined) ||
                            (node.exports.png?.error === undefined &&
                                (node.exports.png?.value === undefined ||
                                    pngDimensions(node.exports.png.value) ===
                                        undefined)))))) ||
            !Array.isArray(node.errors) ||
            node.errors.some(
                (error) =>
                    !error ||
                    typeof error.property !== "string" ||
                    typeof error.message !== "string",
            ) ||
            ids.has(node.id)
        )
            throw new Error("Invalid or duplicate source node");
        ids.add(node.id);
        if (node.children !== undefined) {
            if (!Array.isArray(node.children))
                throw new Error("Invalid source children");
            for (const child of node.children) visit(child);
        }
    }
    visit(value.root);
    if (value.variables !== undefined) validateVariableLibrary(value.variables);
    if (value.components !== undefined)
        validateComponentLibrary<SourceNode<SourceBytes>>(
            value.components,
            (node) => {
                ids.clear();
                visit(node);
            },
        );
}
