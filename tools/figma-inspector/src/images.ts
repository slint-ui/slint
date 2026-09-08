// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { SourceBytes } from "./plugin/source";

export function isPngByteArray(value: unknown): value is Uint8Array {
    if (value instanceof Uint8Array) return true;
    return (
        ArrayBuffer.isView(value) &&
        Object.prototype.toString.call(value) === "[object Uint8Array]"
    );
}

export function pngDimensions(
    value: Uint8Array | readonly number[],
): { readonly width: number; readonly height: number } | undefined {
    if (
        value.length < 33 ||
        ![137, 80, 78, 71, 13, 10, 26, 10].every(
            (byte, index) => value[index] === byte,
        ) ||
        value[8] !== 0 ||
        value[9] !== 0 ||
        value[10] !== 0 ||
        value[11] !== 13 ||
        value[12] !== 73 ||
        value[13] !== 72 ||
        value[14] !== 68 ||
        value[15] !== 82
    )
        return undefined;
    const uint32 = (offset: number) =>
        value[offset] * 16777216 +
        value[offset + 1] * 65536 +
        value[offset + 2] * 256 +
        value[offset + 3];
    const width = uint32(16);
    const height = uint32(20);
    return width > 0 && height > 0 ? { width, height } : undefined;
}

// Read intrinsic dimensions without a browser, network, or Figma host.
export function imageDimensions(
    bytes: Uint8Array,
): { width: number; height: number } | undefined {
    const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
    const size = (width: number, height: number) =>
        width > 0 && height > 0 ? { width, height } : undefined;
    if (
        bytes.length >= 33 &&
        [137, 80, 78, 71, 13, 10, 26, 10].every((b, i) => bytes[i] === b) &&
        view.getUint32(8) === 13 &&
        view.getUint32(12) === 0x49484452
    )
        return size(view.getUint32(16), view.getUint32(20));
    if (
        bytes.length >= 13 &&
        bytes[0] === 71 &&
        bytes[1] === 73 &&
        bytes[2] === 70 &&
        bytes[3] === 56 &&
        (bytes[4] === 55 || bytes[4] === 57) &&
        bytes[5] === 97
    )
        return size(view.getUint16(6, true), view.getUint16(8, true));
    if (bytes[0] !== 0xff || bytes[1] !== 0xd8) return undefined;
    let offset = 2;
    while (offset + 1 < bytes.length) {
        if (bytes[offset++] !== 0xff) return undefined;
        while (bytes[offset] === 0xff) offset++;
        const marker = bytes[offset++];
        if (marker === undefined || marker === 0xda || marker === 0xd9)
            return undefined;
        if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) continue;
        if (offset + 2 > bytes.length) return undefined;
        const length = view.getUint16(offset);
        if (length < 2 || offset + length > bytes.length) return undefined;
        if (
            [
                0xc0, 0xc1, 0xc2, 0xc3, 0xc5, 0xc6, 0xc7, 0xc9, 0xca, 0xcb,
                0xcd, 0xce, 0xcf,
            ].includes(marker)
        ) {
            if (length < 8) return undefined;
            return size(view.getUint16(offset + 5), view.getUint16(offset + 3));
        }
        offset += length;
    }
    return undefined;
}

/** Immutable captured bytes only: placement, masks and raster dependencies are
 * deliberately outside this revision-scoped cache. */
export class NormalizationAssets {
    private readonly arrays = new WeakMap<SourceBytes, Uint8Array>();
    private readonly encoded = new WeakMap<Uint8Array, string>();
    public encodings = 0;
    public encodedBytes = 0;
    bytes(value: SourceBytes): Uint8Array {
        if (value instanceof Uint8Array) return value;
        let bytes = this.arrays.get(value);
        if (!bytes) {
            bytes = new Uint8Array(value);
            this.arrays.set(value, bytes);
        }
        return bytes;
    }
    encode = (bytes: Uint8Array): string => {
        let encoded = this.encoded.get(bytes);
        if (encoded === undefined) {
            encoded = bytesToBase64(bytes);
            this.encoded.set(bytes, encoded);
            this.encodings++;
            this.encodedBytes += bytes.byteLength;
        }
        return encoded;
    };
}

const BASE64_ALPHABET =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

export function normalizeSvg(value: string): string {
    return value.replaceAll("\r\n", "\n").replaceAll("\r", "\n").trim();
}

function svgAttribute(
    attributes: string,
    name: string,
): { readonly value: string } | undefined {
    const match = attributes.match(
        new RegExp(`\\b${name}\\s*=\\s*(['"])([^'"]*)\\1`, "iu"),
    );
    return match === null || match[1] === undefined || match[2] === undefined
        ? undefined
        : { value: match[2] };
}

function setSvgAttribute(
    attributes: string,
    name: string,
    value: string,
): string {
    const pattern = new RegExp(`\\b${name}\\s*=\\s*(['"])([^'"]*)\\1`, "iu");
    if (!pattern.test(attributes))
        return `${attributes}${attributes === "" ? "" : " "}${name}="${value}"`;
    return attributes.replace(pattern, `${name}="${value}"`);
}

/**
 * Figma's SVG export can crop the root viewport to the painted contents while
 * the captured node still has a larger layout box. Re-fitting that cropped
 * viewport scales the path itself. Restore the node-sized viewport without
 * changing any child coordinates so path weight remains identical to Figma.
 */
export function normalizeSvgToNodeBounds(
    value: string,
    width: number,
    height: number,
): string {
    if (!Number.isFinite(width) || !Number.isFinite(height)) return value;
    const normalized = normalizeSvg(value);
    const openingTag = normalized.match(/^(<svg\b)([^>]*>)/iu);
    if (
        openingTag === null ||
        openingTag[1] === undefined ||
        openingTag[2] === undefined
    )
        return normalized;

    const rawAttributes = openingTag[2].slice(0, -1);
    const selfClosing = /\/\s*$/u.test(rawAttributes);
    const attributes = selfClosing
        ? rawAttributes.replace(/\/\s*$/u, "").trim()
        : rawAttributes.trim();
    const originalViewBox = svgAttribute(attributes, "viewBox")?.value;
    const viewBoxNumbers = originalViewBox
        ?.trim()
        .split(/[\s,]+/u)
        .map(Number);
    const parsedMinX = viewBoxNumbers?.[0];
    const parsedMinY = viewBoxNumbers?.[1];
    const parsedWidth = viewBoxNumbers?.[2];
    const parsedHeight = viewBoxNumbers?.[3];
    const minX =
        typeof parsedMinX === "number" && Number.isFinite(parsedMinX)
            ? parsedMinX
            : 0;
    const minY =
        typeof parsedMinY === "number" && Number.isFinite(parsedMinY)
            ? parsedMinY
            : 0;
    const sourceWidth =
        typeof parsedWidth === "number" && Number.isFinite(parsedWidth)
            ? parsedWidth
            : width;
    const sourceHeight =
        typeof parsedHeight === "number" && Number.isFinite(parsedHeight)
            ? parsedHeight
            : height;
    // contentsOnly SVG exports use the painted-content bounds as their
    // coordinate system. Keep those source units at 1:1 and center that
    // viewport in the captured node instead of anchoring it at the top-left.
    // This preserves Figma's path weight and leaves antialiased edge pixels
    // inside the node viewport.
    const offsetX = (width - sourceWidth) / 2;
    const offsetY = (height - sourceHeight) / 2;
    const nodeViewBox = `${minX - offsetX} ${minY - offsetY} ${width} ${height}`;
    const withWidth = setSvgAttribute(attributes, "width", String(width));
    const withHeight = setSvgAttribute(withWidth, "height", String(height));
    const withViewBox = setSvgAttribute(withHeight, "viewBox", nodeViewBox);
    if (selfClosing) return `${openingTag[1]} ${withViewBox}/>`;
    return `${openingTag[1]} ${withViewBox}>${normalized.slice(openingTag[0].length)}`;
}

export function utf8ToBase64(value: string): string {
    // Size exactly, including surrogate pairs, without a host TextEncoder.
    let byteLength = 0;
    for (let offset = 0; offset < value.length; offset++) {
        const codePoint = value.codePointAt(offset)!;
        if (codePoint > 0xffff) offset++;
        byteLength +=
            codePoint <= 0x7f
                ? 1
                : codePoint <= 0x7ff
                  ? 2
                  : codePoint <= 0xffff
                    ? 3
                    : 4;
    }
    const bytes = new Uint8Array(byteLength);
    let index = 0;
    for (let offset = 0; offset < value.length; offset++) {
        const codePoint = value.codePointAt(offset)!;
        if (codePoint > 0xffff) offset++;
        if (codePoint <= 0x7f) {
            bytes[index++] = codePoint;
        } else if (codePoint <= 0x7ff) {
            bytes[index++] = 0xc0 | (codePoint >> 6);
            bytes[index++] = 0x80 | (codePoint & 0x3f);
        } else if (codePoint <= 0xffff) {
            bytes[index++] = 0xe0 | (codePoint >> 12);
            bytes[index++] = 0x80 | ((codePoint >> 6) & 0x3f);
            bytes[index++] = 0x80 | (codePoint & 0x3f);
        } else {
            bytes[index++] = 0xf0 | (codePoint >> 18);
            bytes[index++] = 0x80 | ((codePoint >> 12) & 0x3f);
            bytes[index++] = 0x80 | ((codePoint >> 6) & 0x3f);
            bytes[index++] = 0x80 | (codePoint & 0x3f);
        }
    }
    return bytesToBase64(bytes);
}

// Two output characters per lookup; fixed-size chunks bound string growth in
// the Figma sandbox as well as browsers. No host-specific encoder is required.
const BASE64_PAIRS = Array.from(
    { length: 4096 },
    (_, value) => BASE64_ALPHABET[value >> 6] + BASE64_ALPHABET[value & 63],
);

export function bytesToBase64(bytes: Uint8Array): string {
    const chunks: string[] = [];
    const complete = bytes.length - (bytes.length % 3);
    let index = 0;
    while (index < complete) {
        const end = Math.min(index + 12288, complete);
        let chunk = "";
        for (; index < end; index += 3) {
            const value =
                (bytes[index] << 16) |
                (bytes[index + 1] << 8) |
                bytes[index + 2];
            chunk += BASE64_PAIRS[value >>> 12] + BASE64_PAIRS[value & 4095];
        }
        chunks.push(chunk);
    }
    if (index < bytes.length) {
        const second = bytes[index + 1];
        const value = (bytes[index] << 16) | ((second ?? 0) << 8);
        chunks.push(
            BASE64_PAIRS[value >>> 12] +
                (second === undefined
                    ? "=="
                    : `${BASE64_ALPHABET[(value >>> 6) & 63]}=`),
        );
    }
    return chunks.join("");
}
