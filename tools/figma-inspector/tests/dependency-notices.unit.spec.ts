// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFileSync } from "node:child_process";
import { readFile, readdir } from "node:fs/promises";
import { resolve } from "node:path";
import { expect, test, vi } from "vitest";
import { dependencyNotices } from "../scripts/dependency-notices.mjs";

vi.mock("node:child_process", () => ({ execFileSync: vi.fn() }));
vi.mock("node:fs/promises", () => ({ readFile: vi.fn(), readdir: vi.fn() }));

const repoRoot = resolve(import.meta.dirname, "../../..");

test("notices use the repository for metadata and license texts", async () => {
    vi.mocked(execFileSync).mockReturnValue(
        JSON.stringify({
            packages: [
                {
                    id: "runtime",
                    name: "slint-wasm-interpreter",
                    version: "1.19.0",
                    license: "MIT",
                    manifest_path: `${repoRoot}/api/wasm-interpreter/Cargo.toml`,
                },
            ],
            resolve: { nodes: [{ id: "runtime", deps: [] }] },
        }),
    );
    vi.mocked(readdir).mockImplementation(async (path) => {
        if (String(path) === `${repoRoot}/LICENSES`)
            return ["MIT.txt"] as never;
        return [];
    });
    vi.mocked(readFile).mockResolvedValue("Runtime license");

    const notices = await dependencyNotices([]);

    expect(execFileSync).toHaveBeenCalledWith(
        "cargo",
        [
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
            `${repoRoot}/api/wasm-interpreter/Cargo.toml`,
        ],
        expect.any(Object),
    );
    expect(readFile).toHaveBeenCalledWith(
        `${repoRoot}/LICENSES/MIT.txt`,
        "utf8",
    );
    expect(notices.text).toContain("Runtime license");
});
