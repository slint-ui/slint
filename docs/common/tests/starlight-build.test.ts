// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import {
    mkdtemp,
    mkdir,
    readFile,
    rm,
    symlink,
    writeFile,
} from "node:fs/promises";
import { createRequire } from "node:module";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "node:test";

const run = promisify(execFile);
const require = createRequire(import.meta.url);
const workspaceModules = fileURLToPath(
    new URL("../../../node_modules/", import.meta.url),
);
const astroCli = join(
    dirname(require.resolve("astro/package.json")),
    "bin/astro.mjs",
);
const sharedConfig = new URL("../src/utils/astro-config.ts", import.meta.url)
    .href;

test("renders Starlight YAML configuration with a different major at the workspace root", async (t) => {
    const root = await mkdtemp(
        fileURLToPath(new URL("../.starlight-build-", import.meta.url)),
    );
    t.after(() => rm(root, { recursive: true, force: true }));
    const modules = join(root, "node_modules");
    await mkdir(modules);
    const nodeCliRequire = createRequire(
        join(workspaceModules, "@napi-rs/cli/package.json"),
    );
    const yamlDirectory = dirname(
        nodeCliRequire.resolve("js-yaml/package.json"),
    );
    await symlink(yamlDirectory, join(modules, "js-yaml"), "junction");
    await writeFile(
        join(root, "package.json"),
        JSON.stringify({ type: "module" }),
    );
    await mkdir(join(root, "src/pages"), { recursive: true });
    const headSchema = join(
        workspaceModules,
        "@astrojs/starlight/dist/schemas/head.js",
    );
    await writeFile(
        join(root, "src/pages/index.astro"),
        `---
import { HeadConfigSchema } from ${JSON.stringify(headSchema)};
import { message } from "fixture-message";
const result = HeadConfigSchema({ source: "frontmatter" }).safeParse([
    { tag: "meta", content: message },
]);
if (result.success) throw new Error("Expected an invalid meta tag");
---
<p>{result.error.issues[0].message}</p>
`,
    );

    await writeFile(
        join(root, "src/message.ts"),
        'export const message = "YAML regression";\n',
    );

    async function config(source: string, name: string) {
        await writeFile(
            join(root, "astro.config.mjs"),
            `import { ${name} } from ${JSON.stringify(source)};
export default ${name}({
    site: "https://example.com",
    base: "/docs/",
    vite: { resolve: { alias: { "fixture-message": new URL("./src/message.ts", import.meta.url).pathname } } },
});
`,
        );
    }
    function build() {
        return run(process.execPath, [astroCli, "build"], {
            cwd: root,
            maxBuffer: 1024 * 1024,
        });
    }

    // #13989: Externalized Starlight imports resolve to the Node tooling's YAML major.
    await config("astro/config", "defineConfig");
    await assert.rejects(
        build(),
        (error: Error & { stdout: string; stderr: string }) => {
            assert.match(
                error.stdout + error.stderr,
                /does not provide an export named 'default'/,
            );
            return true;
        },
    );
    await config(sharedConfig, "defineSlintDocsConfig");
    await build();
    const page = await readFile(join(root, "dist/index.html"), "utf8");
    assert.match(page, /content: YAML regression/);
});
