// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { test, expect } from "vitest";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const dirname = path.dirname(fileURLToPath(import.meta.url));
const nodeDir = path.join(dirname, "..");

const [major, minor] = process.versions.node.split(".").map(Number);
const hasBuildSea = major > 25 || (major === 25 && minor >= 5);

const files: Record<string, string | Buffer> = {
    "app/package.json": JSON.stringify({
        name: "pack-test",
        type: "module",
        main: "src/main.ts",
        dependencies: { "slint-ui": "*", greeter: "*" },
        devDependencies: { "dev-only": "*" },
    }),
    "app/ui/app.slint": `
        import { Button } from "std-widgets.slint";
        import { Label } from "widgets/label.slint";
        import "fonts/NotoSans-Regular.ttf";
        export component App inherits Window {
            out property <int> image-width: img.source.width;
            img := Image { source: @image-url("images/rgb.png"); }
            Label { }
            Button { text: "Button"; }
        }`,
    "app/ui/widgets/label.slint": `
        export component Label inherits Text { font-family: "Noto Sans"; }`,
    "app/ui/loaded.slint": `
        export component Loaded { out property <image> img: @image-url("images/rgb.png"); }`,
    "app/ui/images/rgb.png": fs.readFileSync(
        path.join(dirname, "resources", "rgb.png"),
    ),
    "app/ui/fonts/NotoSans-Regular.ttf": fs.readFileSync(
        path.join(
            nodeDir,
            "../../tests/screenshots/fonts/NotoSans-Regular.ttf",
        ),
    ),
    // Resolves the `.slint` file from its own `import.meta.url`, in a subdirectory.
    "app/src/lib/load.ts": `
        import * as slint from "slint-ui";
        export function loadLoaded(): any {
            return slint.loadFile(new URL("../../ui/loaded.slint", import.meta.url));
        }`,
    "app/src/main.ts": `#!/usr/bin/env node
        import * as fs from "node:fs";
        import { private_api } from "slint-ui";
        import { greet } from "greeter";
        import { App } from "../ui/app.slint";
        import { loadLoaded } from "./lib/load.ts";
        private_api.initTesting();
        const app = new (App as any)();
        const loaded = new (loadLoaded().Loaded)();
        const shared = new URL("../../shared/greeting.txt", import.meta.url);
        const greeting = fs.existsSync(shared) ? fs.readFileSync(shared, "utf8") : undefined;
        console.log(JSON.stringify({ image: app.image_width, loaded: loaded.img.width, greet: greet(), greeting }));
        await Promise.resolve();
        process.exit(0);`,
    "app/node_modules/greeter/package.json": JSON.stringify({
        name: "greeter",
        type: "module",
        main: "index.js",
        dependencies: { word: "*" },
    }),
    "app/node_modules/greeter/index.js": `import { word } from "word"; export const greet = () => word;`,
    "app/node_modules/word/package.json": JSON.stringify({
        name: "word",
        type: "module",
        main: "index.js",
    }),
    "app/node_modules/word/index.js": `export const word = "hi";`,
    "app/node_modules/dev-only/package.json": JSON.stringify({
        name: "dev-only",
    }),
    "shared/greeting.txt": "hello",
};

/** Write the application, and a directory next to it, into a temporary directory. */
function writeApp(): string {
    const root = fs.mkdtempSync(path.join(os.tmpdir(), "slint-pack-"));
    for (const [file, contents] of Object.entries(files)) {
        fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
        fs.writeFileSync(path.join(root, file), contents);
    }
    fs.symlinkSync(
        nodeDir,
        path.join(root, "app", "node_modules", "slint-ui"),
        "junction",
    );
    return root;
}

/** Pack the application in `root`, and return the directory it wrote and the executable. */
function pack(root: string, ...args: string[]): [string, string] {
    execFileSync(
        process.execPath,
        [path.join(nodeDir, "dist", "cli.js"), "pack", ...args],
        { cwd: path.join(root, "app"), stdio: "pipe" },
    );
    const out = path.join(
        root,
        "app",
        "out",
        `pack-test-${process.platform}-${process.arch}`,
    );
    switch (process.platform) {
        case "darwin":
            return [
                out,
                path.join(
                    out,
                    "pack-test.app",
                    "Contents",
                    "MacOS",
                    "pack-test",
                ),
            ];
        case "win32":
            return [out, path.join(out, "pack-test.exe")];
        default:
            return [out, path.join(out, "pack-test")];
    }
}

/** Run the executable with the sources gone, so that it reads what it carries. */
function runWithoutSources(root: string, executable: string) {
    for (const dir of ["app/ui", "app/src", "app/node_modules", "shared"]) {
        fs.rmSync(path.join(root, dir), { recursive: true });
    }
    return JSON.parse(execFileSync(executable, { encoding: "utf8" }));
}

test.skipIf(!hasBuildSea)(
    "a packed application carries its files and production dependencies",
    { timeout: 120_000 },
    () => {
        const root = writeApp();
        try {
            const [out, executable] = pack(root);
            const packed = fs.readdirSync(out, { recursive: true }).map(String);
            expect(packed.some((file) => file.includes("dev-only"))).toBe(
                false,
            );
            expect(runWithoutSources(root, executable)).toStrictEqual({
                image: 64,
                loaded: 64,
                greet: "hi",
            });
        } finally {
            fs.rmSync(root, { recursive: true, force: true });
        }
    },
);

test.skipIf(!hasBuildSea)(
    "a packed application reads its --asset files",
    { timeout: 120_000 },
    () => {
        const root = writeApp();
        try {
            const [, executable] = pack(root, "--asset", "../shared");
            expect(runWithoutSources(root, executable)).toStrictEqual({
                image: 64,
                loaded: 64,
                greet: "hi",
                greeting: "hello",
            });
        } finally {
            fs.rmSync(root, { recursive: true, force: true });
        }
    },
);
