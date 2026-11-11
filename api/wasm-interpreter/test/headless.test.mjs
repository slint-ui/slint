// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createRequire } from "node:module";
import { test } from "node:test";

const interpreter = createRequire(import.meta.url)(
    "../pkg-mcp/slint_wasm_interpreter.js",
);
const repository = new URL("../../../", import.meta.url);

test("Headless Wasm renders text, an SVG, and a raster image in plain Node", async () => {
    const source = `
        import { CheckBox } from "std-widgets.slint";
        import "${new URL("tests/screenshots/fonts/NotoSans-Regular.ttf", repository)}";
        export component Preview inherits Window {
            width: 128px;
            height: 104px;
            background: white;
            Image {
                x: 8px; y: 8px; width: 40px; height: 24px;
                source: @image-url("${new URL("examples/iot-dashboard/images/up.svg", repository)}");
            }
            Image {
                x: 72px; y: 8px; width: 40px; height: 24px;
                source: @image-url("${new URL("examples/iot-dashboard/images/up.png", repository)}");
            }
            Text {
                x: 8px; y: 40px; width: 112px; height: 28px;
                text: "Hello Slint"; color: black; font-family: "Noto Sans"; font-size: 20px;
            }
            CheckBox {
                x: 8px; y: 76px; width: 112px; height: 20px;
                checked: true; text: "Built-in icon";
            }
        }
    `;
    const loadedResources = [];
    const compilation = await interpreter.compile_from_string_with_style(
        source,
        new URL("preview.slint", repository).href,
        "fluent",
        async (url) => readFile(new URL(url), "utf8").catch(() => null),
        async (url) => {
            loadedResources.push(url);
            return new Uint8Array(await readFile(new URL(url)));
        },
        undefined,
        undefined,
    );
    assert.deepEqual(compilation.diagnostics, []);
    const component = compilation.component;
    assert.ok(component, compilation.error_string);
    const frame = component.render();
    assert.equal(frame.width, 128);
    assert.equal(frame.height, 104);
    const pixels = frame.pixels;
    const isIconPixel = (red, green, blue) =>
        Math.abs(red - 146) < 3 &&
        Math.abs(green - 156) < 3 &&
        Math.abs(blue - 178) < 3;
    const isTextPixel = (red, green, blue) =>
        red < 80 && green < 80 && blue < 80;
    for (const [name, startX, startY, width, height, matches] of [
        ["SVG", 8, 8, 40, 24, isIconPixel],
        ["raster", 72, 8, 40, 24, isIconPixel],
        ["text", 8, 40, 112, 28, isTextPixel],
    ]) {
        let matchingPixels = 0;
        for (let row = startY; row < startY + height; row++) {
            for (let column = startX; column < startX + width; column++) {
                const offset = (row * frame.width + column) * 4;
                if (
                    pixels[offset + 3] === 255 &&
                    matches(...pixels.subarray(offset, offset + 3))
                )
                    matchingPixels++;
            }
        }
        assert.ok(
            matchingPixels > 20,
            `${name} must appear in rendered pixels, found ${matchingPixels}`,
        );
    }
    for (const extension of [".ttf", ".svg", ".png"]) {
        assert.ok(
            loadedResources.some((url) => url.endsWith(extension)),
            `${extension} must load through JavaScript`,
        );
    }
    frame.free();
    component.free();
    compilation.free();
});
