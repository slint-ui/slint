// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Follows docs/nodejs/src/content/docs/packaging.mdx.

import { describe, test, expect } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

const dirname = path.dirname(fileURLToPath(import.meta.url));
const require = createRequire(import.meta.url);

const [major, minor] = process.versions.node.split(".").map(Number);
const hasBuildSea = major > 25 || (major === 25 && minor >= 5);

const output = process.platform === "win32" ? "myapp.exe" : "myapp";

const app: Record<string, string | Buffer> = {
    "package.json": JSON.stringify({ type: "module" }),
    "ui/app.slint": `
        export component App inherits Window {
            out property <int> image-width: img.source.width;
            img := Image { source: @image-url("rgb.png"); }
        }`,
    "ui/loaded.slint": `
        export component Loaded { out property <image> img: @image-url("rgb.png"); }`,
    "ui/rgb.png": fs.readFileSync(path.join(dirname, "resources", "rgb.png")),
    "main.ts": `
        import * as slint from "slint-ui";
        import { App } from "./ui/app.slint";
        slint.private_api.initTesting();
        const app = new (App as any)();
        const { Loaded } = slint.loadFile(new URL("./ui/loaded.slint", import.meta.url)) as any;
        // Top-level await, as in a typical \`await window.run()\`.
        await Promise.resolve();
        console.log(JSON.stringify({ image: app.image_width, loaded: new Loaded().img.width }));`,
};

/**
 * Write `app` and `files`, run their `build.mjs` if any, build the executable,
 * and run it without `leftOut`, from another directory.
 */
function packageAndRun(
    files: Record<string, string | Buffer>,
    leftOut: string[] = [],
) {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "slint-sea-"));
    try {
        for (const [file, contents] of Object.entries({ ...app, ...files })) {
            fs.mkdirSync(path.dirname(path.join(root, file)), {
                recursive: true,
            });
            fs.writeFileSync(path.join(root, file), contents);
        }
        fs.mkdirSync(path.join(root, "node_modules"));
        for (const [name, dir] of [
            ["slint-ui", path.join(dirname, "..")],
            ["esbuild", path.dirname(require.resolve("esbuild/package.json"))],
        ]) {
            fs.symlinkSync(
                dir,
                path.join(root, "node_modules", name),
                "junction",
            );
        }

        if ("build.mjs" in files) {
            execFileSync(process.execPath, ["build.mjs"], { cwd: root });
        }
        execFileSync(process.execPath, ["--build-sea", "sea-config.json"], {
            cwd: root,
        });
        const executable = path.join(root, output);
        if (process.platform === "darwin") {
            execFileSync("codesign", ["--sign", "-", "--force", executable]);
        }

        for (const file of leftOut) {
            fs.rmSync(path.join(root, file), { recursive: true });
        }
        return JSON.parse(
            execFileSync(executable, { cwd: os.tmpdir(), encoding: "utf8" }),
        );
    } finally {
        // Windows holds on to a file of a process that just exited for a moment.
        fs.rmSync(root, { recursive: true, force: true, maxRetries: 10 });
    }
}

describe.skipIf(!hasBuildSea)("a single executable application", () => {
    test("bundled with esbuild runs with its addon and .slint files", {
        timeout: 120_000,
    }, () => {
        const files = {
            "build.mjs": `
                import * as esbuild from "esbuild";
                import { slint } from "slint-ui/esbuild";

                await esbuild.build({
                    entryPoints: ["main.ts"],
                    bundle: true,
                    platform: "node",
                    format: "esm",
                    outfile: "main.bundle.mjs",
                    plugins: [slint()],
                });`,
            "sea-config.json": JSON.stringify({
                main: "main.bundle.mjs",
                mainFormat: "module",
                output,
                disableExperimentalSEAWarning: true,
            }),
        };
        expect(
            packageAndRun(files, [
                "node_modules",
                "main.ts",
                "main.bundle.mjs",
            ]),
        ).toStrictEqual({ image: 64, loaded: 64 });
    });

    test("without a bundler runs from the files next to it", {
        timeout: 120_000,
    }, () => {
        const files = {
            "start.cjs": `import("slint-ui/register").then(() => import("./main.ts"));`,
            "sea-main.cjs": `require("node:module").createRequire(__filename)("./start.cjs");`,
            "sea-config.json": JSON.stringify({
                main: "sea-main.cjs",
                output,
                disableExperimentalSEAWarning: true,
            }),
        };
        expect(packageAndRun(files)).toStrictEqual({ image: 64, loaded: 64 });
    });
});
