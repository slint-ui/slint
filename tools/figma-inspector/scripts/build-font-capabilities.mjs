// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
// cspell:words cmap subtable fvar wght

import { readFileSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { runtimeRoot, runtimePin, verifyRuntime } from "./runtime-pin.mjs";

verifyRuntime();
const source = readFileSync(
    resolve(runtimeRoot, "internal/common/sharedfontique.rs"),
    "utf8",
).split("#[cfg(test)]")[0];
const paths = [
    ...source.matchAll(/include_bytes!\("sharedfontique\/([^"]+)"\)/g),
].map((match) => match[1]);
if (!paths.length) throw Error("No embedded runtime fonts found");
const fonts = [...new Set(paths)].map((path) => {
    const data = readFileSync(
        resolve(runtimeRoot, "internal/common/sharedfontique", path),
    );
    const tables = new Map();
    for (let i = 0; i < data.readUInt16BE(4); i++) {
        const entry = 12 + i * 16;
        tables.set(
            data.toString("ascii", entry, entry + 4),
            data.readUInt32BE(entry + 8),
        );
    }
    const table = (name) => {
        const offset = tables.get(name);
        if (offset === undefined) throw Error(`Missing font table ${name}`);
        return offset;
    };
    const name = table("name");
    const families = new Set();
    for (let i = 0; i < data.readUInt16BE(name + 2); i++) {
        const entry = name + 6 + i * 12;
        const platform = data.readUInt16BE(entry);
        const id = data.readUInt16BE(entry + 6);
        if (![1, 16].includes(id) || ![0, 3].includes(platform)) continue;
        const start =
            name + data.readUInt16BE(name + 4) + data.readUInt16BE(entry + 10);
        const length = data.readUInt16BE(entry + 8);
        let family = "";
        for (let offset = start; offset < start + length; offset += 2)
            family += String.fromCharCode(data.readUInt16BE(offset));
        families.add(family);
    }
    if (!families.size) throw Error("Missing embedded font family");
    const glyphs = new Set();
    const cmap = table("cmap");
    for (let i = 0; i < data.readUInt16BE(cmap + 2); i++) {
        const entry = cmap + 4 + i * 8;
        const platform = data.readUInt16BE(entry);
        const encoding = data.readUInt16BE(entry + 2);
        if (platform !== 0 && !(platform === 3 && [1, 10].includes(encoding)))
            continue;
        const subtable = cmap + data.readUInt32BE(entry + 4);
        const format = data.readUInt16BE(subtable);
        if (format === 12) {
            for (
                let group = 0;
                group < data.readUInt32BE(subtable + 12);
                group++
            ) {
                const record = subtable + 16 + group * 12;
                const start = data.readUInt32BE(record);
                const end = data.readUInt32BE(record + 4);
                const firstGlyph = data.readUInt32BE(record + 8);
                for (let cp = start; cp <= end; cp++)
                    if (firstGlyph + cp - start !== 0) glyphs.add(cp);
            }
        } else if (format === 4) {
            const count = data.readUInt16BE(subtable + 6) / 2;
            for (let segment = 0; segment < count; segment++) {
                const end = data.readUInt16BE(subtable + 14 + segment * 2);
                const start = data.readUInt16BE(
                    subtable + 16 + count * 2 + segment * 2,
                );
                const delta = data.readInt16BE(
                    subtable + 16 + count * 4 + segment * 2,
                );
                const rangeAddress = subtable + 16 + count * 6 + segment * 2;
                const range = data.readUInt16BE(rangeAddress);
                for (let cp = start; cp <= end && cp !== 0xffff; cp++) {
                    const glyph =
                        range === 0
                            ? (cp + delta) & 0xffff
                            : data.readUInt16BE(
                                  rangeAddress + range + (cp - start) * 2,
                              );
                    if (
                        glyph !== 0 &&
                        (range === 0 || ((glyph + delta) & 0xffff) !== 0)
                    )
                        glyphs.add(cp);
                }
            }
        } else throw Error(`Unsupported embedded font cmap format ${format}`);
    }
    if (!glyphs.size) throw Error("Missing embedded font glyph coverage");
    const ranges = [];
    for (const cp of [...glyphs].sort((a, b) => a - b)) {
        const last = ranges.at(-1);
        if (last && last[1] + 1 === cp) last[1] = cp;
        else ranges.push([cp, cp]);
    }
    const axes = {};
    if (tables.has("fvar")) {
        const fvar = table("fvar");
        for (let i = 0; i < data.readUInt16BE(fvar + 8); i++) {
            const record =
                fvar +
                data.readUInt16BE(fvar + 4) +
                i * data.readUInt16BE(fvar + 10);
            const tag = data.toString("ascii", record, record + 4);
            axes[tag] = {
                min: data.readInt32BE(record + 4) / 65536,
                default: data.readInt32BE(record + 8) / 65536,
                max: data.readInt32BE(record + 12) / 65536,
            };
        }
    }
    const os2 = table("OS/2");
    return {
        families: [...families].sort(),
        italic: (data.readUInt16BE(os2 + 62) & 1) !== 0,
        weight: data.readUInt16BE(os2 + 4),
        axes,
        ranges,
    };
});
const manifest = resolve(
    import.meta.dirname,
    "../src/plugin/runtime-fonts.json",
);
const expected = { revision: runtimePin.revision, fonts };
if (process.argv.includes("--update"))
    writeFileSync(manifest, `${JSON.stringify(expected, null, 4)}\n`);
else if (
    JSON.stringify(JSON.parse(readFileSync(manifest, "utf8"))) !==
    JSON.stringify(expected)
)
    throw Error(
        "Embedded font capabilities changed; run node scripts/build-font-capabilities.mjs --update and review the manifest",
    );
