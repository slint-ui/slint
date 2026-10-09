// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { chromium } from "playwright";

test("fullscreen controls and panels fill the tab and inline views retain their width limit", async context => {
  const browser = await chromium.launch();
  context.after(() => browser.close());
  const page = await browser.newPage();
  const template = await readFile(new URL("../preview/index.html", import.meta.url), "utf8");
  await page.setContent(template.replace(/<script\b[^>]*>[\s\S]*?<\/script>/g, ""));
  const zoom = await readFile(new URL("../preview/zoom.mjs", import.meta.url), "utf8");
  await page.addScriptTag({ content: zoom.replace("export function", "function") + `
    const canvas = document.getElementById("slint-preview");
    canvas.hidden = false;
    canvas.style.width = "320px";
    canvas.style.height = "160px";
    document.getElementById("status").hidden = true;
    document.getElementById("version").textContent = "v0.20.0";
    window.previewZoom = installPreviewZoom({ canvas, initialSize: { width: 320, height: 160 } });
  ` });
  for (const { width, height } of [{ width: 1280, height: 800 }, { width: 1600, height: 1000 }, { width: 500, height: 700 }]) {
    await page.setViewportSize({ width, height });
    for (const displayMode of ["inline", "fullscreen", "inline"]) {
      await page.evaluate(mode => window.previewZoom.setDisplayMode(mode), displayMode);
      const expectedWidth = displayMode === "fullscreen" ? width : Math.min(width, 720);
      const bounds = await page.evaluate(() => {
        return Object.fromEntries(["view-controls", "preview-panel", "version", "preview-scroll", "preview-content"].map(name => {
          const { width, height, left } = document.getElementById(name).getBoundingClientRect();
          return [name, { width, height, left }];
        }));
      });
      for (const name of ["view-controls", "preview-panel", "version"]) {
        assert.equal(bounds[name].width, expectedWidth, `${displayMode}: ${name} at ${width}px`);
        assert.equal(bounds[name].left, (width - expectedWidth) / 2);
      }
      if (displayMode === "fullscreen") {
        assert(bounds["preview-scroll"].height > 420);
        assert.equal(bounds["preview-content"].width, width - 24);
      } else {
        assert(bounds["preview-scroll"].height <= 420);
        assert.equal(bounds["preview-content"].width, 320);
      }
      await page.evaluate(() => {
        document.getElementById("preview-panel").hidden = true;
        document.getElementById("code-panel").hidden = false;
      });
      const codeWidth = await page.locator("#code-panel").evaluate(element => element.getBoundingClientRect().width);
      assert.equal(codeWidth, expectedWidth, `${displayMode}: code-panel at ${width}px`);
      await page.evaluate(() => {
        document.getElementById("preview-panel").hidden = false;
        document.getElementById("code-panel").hidden = true;
        window.previewZoom.refresh();
      });
    }
  }
});
