// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { SourceBytes, SourceCapture, SourceNode } from "./plugin/source";

export type CaptureAssets = {
    readonly captureAssetVersion: 1;
    readonly captureJson: string;
    readonly captureAssets: readonly Uint8Array[];
};

// Keep the fixture/capture contract JSON-based, but carry bytes separately on
// the live transport. Only known image fields can contain asset references.
export function packCaptureAssets(
    source: SourceCapture<SourceBytes>,
): CaptureAssets {
    const captureAssets: Uint8Array[] = [];
    const captureJson = serializeCapture(source, (bytes) => {
        const index = captureAssets.length;
        captureAssets.push(
            bytes instanceof Uint8Array ? bytes : Uint8Array.from(bytes),
        );
        return index;
    });
    return { captureAssetVersion: 1, captureJson, captureAssets };
}

function serializeCapture(
    source: SourceCapture<SourceBytes>,
    asset: (bytes: SourceBytes, key: string) => number,
): string {
    const reference = (bytes: SourceBytes, key: string) => ({
        $captureAsset: asset(bytes, key),
    });
    const node = (value: SourceNode<SourceBytes>): unknown => ({
        ...value,
        ...(value.children ? { children: value.children.map(node) } : {}),
        ...(value.exports?.png?.value
            ? {
                  exports: {
                      ...value.exports,
                      png: {
                          ...value.exports.png,
                          value: reference(
                              value.exports.png.value,
                              `node:${value.id}`,
                          ),
                      },
                  },
              }
            : {}),
    });
    const images = Object.fromEntries(
        Object.entries(source.images).map(([id, image]) => [
            id,
            image.value
                ? {
                      ...image,
                      value: {
                          ...image.value,
                          bytes: reference(image.value.bytes, `image:${id}`),
                      },
                  }
                : image,
        ]),
    );
    return JSON.stringify({
        ...source,
        root: node(source.root),
        images,
        ...(source.components
            ? {
                  components: {
                      ...source.components,
                      definitions: source.components.definitions.map((d) => ({
                          ...d,
                          variants: d.variants.map((v) => ({
                              ...v,
                              root: node(v.root),
                          })),
                      })),
                  },
              }
            : {}),
    });
}

export function unpackCaptureAssets(
    captureJson: string,
    assets: readonly Uint8Array[],
    representation: "json" | "binary" = "json",
): unknown {
    const source = JSON.parse(captureJson);
    const bytes = (value: unknown): SourceBytes => {
        const index =
            value && typeof value === "object"
                ? (value as { $captureAsset?: unknown }).$captureAsset
                : undefined;
        if (
            typeof index !== "number" ||
            !Number.isSafeInteger(index) ||
            index < 0 ||
            index >= assets.length
        )
            throw Error("Invalid captured image asset reference");
        return representation === "binary"
            ? assets[index]
            : Array.from(assets[index]);
    };
    const node = (value: SourceNode<SourceBytes>) => {
        if (value.exports?.png?.value !== undefined)
            value.exports.png.value = bytes(value.exports.png.value);
        for (const child of value.children ?? []) node(child);
    };
    node(source.root);
    for (const d of source.components?.definitions ?? [])
        for (const v of d.variants) node(v.root);
    for (const image of Object.values(
        source.images,
    ) as SourceCapture<SourceBytes>["images"][string][])
        if (image.value) image.value.bytes = bytes(image.value.bytes);
    return source;
}

export function isCaptureAssets(value: {
    captureAssetVersion?: unknown;
    captureAssets?: unknown;
}): boolean {
    return (
        (value.captureAssetVersion === 1 || value.captureAssetVersion === 2) &&
        Array.isArray(value.captureAssets) &&
        value.captureAssets.every(
            (asset) =>
                isByteArray(asset) ||
                (value.captureAssetVersion === 2 &&
                    typeof asset === "number" &&
                    Number.isSafeInteger(asset) &&
                    asset >= 0),
        )
    );
}

export type CachedCaptureAssets = {
    readonly captureAssetVersion: 2;
    readonly captureJson: string;
    readonly captureAssets: readonly (Uint8Array | number)[];
};

function sameCaptureBytes(bytes: SourceBytes, previous: Uint8Array): boolean {
    if (bytes === previous) return true;
    if (bytes.length !== previous.length) return false;
    let offset = 0;
    if (
        bytes instanceof Uint8Array &&
        bytes.byteOffset % 4 === 0 &&
        previous.byteOffset % 4 === 0
    ) {
        const length = Math.floor(bytes.byteLength / 4);
        const left = new Uint32Array(bytes.buffer, bytes.byteOffset, length);
        const right = new Uint32Array(
            previous.buffer,
            previous.byteOffset,
            length,
        );
        for (let i = 0; i < length; i++) if (left[i] !== right[i]) return false;
        offset = length * 4;
    }
    for (; offset < bytes.length; offset++)
        if (bytes[offset] !== previous[offset]) return false;
    return true;
}

// References address the preceding capture's asset table. Only that table is
// retained, capped at 32 MiB of image bytes per cache (oversize assets resend).
const CACHE_BYTES = 32 * 1024 * 1024;
export class CaptureAssetSender {
    private previous = new Map<string, { index: number; bytes: Uint8Array }>();
    public reset(): void {
        this.previous.clear();
    }
    public pack(source: SourceCapture<SourceBytes>): CachedCaptureAssets {
        const next = new Map<string, { index: number; bytes: Uint8Array }>();
        const captureAssets: (Uint8Array | number)[] = [];
        let retained = 0;
        const captureJson = serializeCapture(source, (bytes, key) => {
            const old = this.previous.get(key);
            const matches =
                old !== undefined && sameCaptureBytes(bytes, old.bytes);
            const data =
                matches && old
                    ? old.bytes
                    : bytes instanceof Uint8Array
                      ? bytes
                      : Uint8Array.from(bytes);
            const index = captureAssets.length;
            captureAssets.push(matches && old ? old.index : data);
            if (retained + data.byteLength <= CACHE_BYTES) {
                retained += data.byteLength;
                next.set(key, { index, bytes: data });
            }
            return index;
        });
        this.previous = next;
        return { captureAssetVersion: 2, captureJson, captureAssets };
    }
}

export class CaptureAssetReceiver {
    private previous: (Uint8Array | undefined)[] = [];
    public resolve(assets: readonly (Uint8Array | number)[]): Uint8Array[] {
        const resolved = assets.map((asset) => {
            const bytes =
                typeof asset === "number" ? this.previous[asset] : asset;
            if (!isByteArray(bytes))
                throw Error("Missing cached capture asset");
            return bytes;
        });
        let retained = 0;
        this.previous = resolved.map((bytes) => {
            if (retained + bytes.byteLength > CACHE_BYTES) return undefined;
            retained += bytes.byteLength;
            return bytes;
        });
        return resolved;
    }
}

function isByteArray(value: unknown): value is Uint8Array {
    return (
        ArrayBuffer.isView(value) &&
        Object.prototype.toString.call(value) === "[object Uint8Array]"
    );
}

// Versioned transport only. Reconstructs the original Slint and snapshot JSON
// byte-for-byte; no image decoding, resizing, or renderer changes.
type Parts = readonly (string | number)[];
export type AssetPreview = {
    readonly assetVersion: 1;
    readonly assets: readonly string[];
    readonly source: Parts;
    readonly snapshotJson: Parts;
};

export function packPreviewAssets(
    source: string,
    snapshotJson = "",
): AssetPreview {
    const assets: string[] = [];
    const indices = new Map<string, number>();
    function split(text: string, pattern: RegExp): Parts {
        const parts: (string | number)[] = [];
        let offset = 0;
        for (
            let match = pattern.exec(text);
            match;
            match = pattern.exec(text)
        ) {
            const start = match.index + match[0].length;
            const end = text.indexOf('"', start);
            if (end < 0) break;
            // Skip the image body entirely when searching for the next asset.
            pattern.lastIndex = end + 1;
            const data = text.slice(start, end);
            // Small inline values cost more to reference than to retain.
            if (data.length < 128) continue;
            let index = indices.get(data);
            if (index === undefined) {
                index = assets.length;
                indices.set(data, index);
                assets.push(data);
            }
            parts.push(text.slice(offset, start), index);
            offset = start + data.length;
        }
        parts.push(text.slice(offset));
        return parts;
    }
    return {
        assetVersion: 1,
        assets,
        source: split(source, /data:image\/(?:png|jpeg|gif|svg\+xml);base64,/g),
        snapshotJson: split(snapshotJson, /"data":"/g),
    };
}

export function unpackPreviewAssets(value: unknown): {
    source: string;
    snapshotJson: string;
} {
    if (!isAssetPreview(value)) throw Error("Invalid asset preview");
    const packed = value;
    function join(parts: Parts): string {
        if (!Array.isArray(parts)) throw Error("Invalid asset preview parts");
        return parts
            .map((part) => {
                if (typeof part === "string") return part;
                if (
                    !Number.isSafeInteger(part) ||
                    part < 0 ||
                    part >= packed.assets.length
                )
                    throw Error("Invalid asset preview reference");
                return packed.assets[part];
            })
            .join("");
    }
    return {
        source: join(packed.source),
        snapshotJson: join(packed.snapshotJson),
    };
}

export function isAssetPreview(value: unknown): value is AssetPreview {
    if (!value || typeof value !== "object") return false;
    const packed = value as AssetPreview;
    if (packed.assetVersion !== 1 || !Array.isArray(packed.assets))
        return false;
    for (const asset of packed.assets)
        if (typeof asset !== "string") return false;
    for (const parts of [packed.source, packed.snapshotJson]) {
        if (!Array.isArray(parts)) return false;
        for (const part of parts)
            if (
                typeof part !== "string" &&
                !(
                    typeof part === "number" &&
                    Number.isSafeInteger(part) &&
                    part >= 0 &&
                    part < packed.assets.length
                )
            )
                return false;
    }
    return true;
}
