// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { isAbsolute, normalize } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { crc32, deflateSync } from "node:zlib";

const require = createRequire(import.meta.url);
const CHECKER_MODULE = "./checker/slint_wasm_interpreter.js";
let checker;

function loadChecker() {
    if (checker) return checker;
    try {
        const interpreter = require(CHECKER_MODULE);
        const { version } = JSON.parse(
            readFileSync(
                new URL("checker/package.json", import.meta.url),
                "utf8",
            ),
        );
        checker = {
            interpreter,
            runtime: `Slint ${version} (MCP Wasm software)`,
        };
        return checker;
    } catch (error) {
        throw new Error(
            `Cannot load the MCP Wasm checker: ${error.message}. Run pnpm build:checker in ai-plugins/mcp/slint-preview before packaging the plugin.`,
        );
    }
}

function localPath(value) {
    if (isAbsolute(value)) return normalize(value).replaceAll("\\", "/");
    const url = new URL(value.replace(/^slint-file:/, "file:"));
    if (url.protocol !== "file:")
        throw new Error(`Unsupported resource URL: ${value}`);
    return normalize(fileURLToPath(url)).replaceAll("\\", "/");
}

// cspell:ignore IHDR IDAT IEND scanlines

function pngChunk(type, data) {
    const payload = Buffer.concat([Buffer.from(type), data]);
    const chunk = Buffer.alloc(payload.length + 8);
    chunk.writeUInt32BE(data.length, 0);
    payload.copy(chunk, 4);
    chunk.writeUInt32BE(crc32(payload), payload.length + 4);
    return chunk;
}

function encodePng(frame) {
    const { width, height } = frame;
    const pixels = Buffer.from(frame.pixels);
    const header = Buffer.alloc(13);
    header.writeUInt32BE(width, 0);
    header.writeUInt32BE(height, 4);
    header[8] = 8;
    header[9] = 6;
    const rowBytes = width * 4;
    const scanlines = Buffer.alloc((rowBytes + 1) * height);
    for (let row = 0; row < height; row++) {
        pixels.copy(
            scanlines,
            row * (rowBytes + 1) + 1,
            row * rowBytes,
            (row + 1) * rowBytes,
        );
    }
    return Buffer.concat([
        Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
        pngChunk("IHDR", header),
        pngChunk("IDAT", deflateSync(scanlines)),
        pngChunk("IEND", Buffer.alloc(0)),
    ]).toString("base64");
}

export async function checkSlint(previewInput) {
    const checkerInstance = loadChecker();
    const { interpreter, runtime } = checkerInstance;
    const readSource = async (value) => {
        const path = localPath(value);
        try {
            return readFileSync(path, "utf8");
        } catch (error) {
            if (error.code === "ENOENT" || error.code === "ENOTDIR")
                return null;
            throw error;
        }
    };
    const readResource = async (value) => {
        if (!previewInput.path)
            throw new Error("Use path to load local images or fonts");
        return new Uint8Array(readFileSync(localPath(value)));
    };
    const result = {
        ok: false,
        status: "compile-error",
        diagnostics: [],
        runtime,
    };
    const baseUrl = previewInput.path
        ? pathToFileURL(previewInput.path).href
        : "file:///main.slint";
    let compilation;
    let component;
    let frame;
    let trapped = false;
    try {
        const source = previewInput.path
            ? readFileSync(previewInput.path, "utf8")
            : previewInput.source;
        compilation = await interpreter.compile_from_string_with_style(
            source,
            baseUrl,
            previewInput.style ?? "",
            previewInput.path ? readSource : async () => null,
            readResource,
            undefined,
            previewInput.projectRoot
                ? pathToFileURL(previewInput.projectRoot).href
                : undefined,
        );
        result.diagnostics = compilation.diagnostics.map(
            ({ level, ...diagnostic }) => ({
                ...diagnostic,
                severity: ["error", "warning", "note"][level] ?? "note",
            }),
        );
        component = compilation.component;
        if (!component) return result;
        result.status = "render-error";
        frame = component.render();
        result.image = {
            png: encodePng(frame),
            width: frame.width,
            height: frame.height,
        };
        result.ok = true;
        result.status = "ready";
        return result;
    } catch (error) {
        if (error instanceof WebAssembly.RuntimeError) {
            trapped = true;
            if (checker === checkerInstance) {
                delete require.cache[require.resolve(CHECKER_MODULE)];
                checker = undefined;
            }
        }
        if (result.status === "render-error")
            result.renderError = String(error.message ?? error);
        else
            result.diagnostics.push({
                severity: "error",
                message: String(error.message ?? error),
                fileName: previewInput.path ?? baseUrl,
                lineNumber: 0,
                columnNumber: 0,
            });
        return result;
    } finally {
        if (!trapped) {
            frame?.free();
            component?.free();
            compilation?.free();
        }
    }
}
