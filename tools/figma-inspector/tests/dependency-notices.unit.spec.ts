// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFileSync } from "node:child_process";
import { readFile, readdir } from "node:fs/promises";
import { expect, test, vi } from "vitest";
import { dependencyNotices } from "../scripts/dependency-notices.mjs";
import { runtimeRoot, verifyRuntime } from "../scripts/runtime-pin.mjs";

vi.mock("node:child_process", () => ({ execFileSync: vi.fn() }));
vi.mock("node:fs/promises", () => ({ readFile: vi.fn(), readdir: vi.fn() }));
vi.mock("../scripts/runtime-pin.mjs", () => ({
    runtimeRoot: "/pinned-runtime",
    verifyRuntime: vi.fn(),
}));

test("notices use the verified runtime checkout for metadata and license texts", async () => {
    vi.mocked(execFileSync).mockReturnValue(
        JSON.stringify({
            packages: [
                {
                    id: "runtime",
                    name: "slint-wasm-interpreter",
                    version: "1.18.0",
                    license: "MIT",
                    manifest_path: `${runtimeRoot}/api/wasm-interpreter/Cargo.toml`,
                },
            ],
            resolve: { nodes: [{ id: "runtime", deps: [] }] },
        }),
    );
    vi.mocked(readdir).mockImplementation(async (path) => {
        if (String(path) === `${runtimeRoot}/LICENSES`)
            return ["MIT.txt"] as never;
        return [];
    });
    vi.mocked(readFile).mockResolvedValue("Pinned runtime license");

    const notices = await dependencyNotices([]);

    expect(verifyRuntime).toHaveBeenCalledOnce();
    expect(execFileSync).toHaveBeenCalledWith(
        "cargo",
        [
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
            `${runtimeRoot}/api/wasm-interpreter/Cargo.toml`,
        ],
        expect.any(Object),
    );
    expect(readFile).toHaveBeenCalledWith(
        `${runtimeRoot}/LICENSES/MIT.txt`,
        "utf8",
    );
    expect(notices.text).toContain("Pinned runtime license");
});

test("notices reject an unverified runtime before running Cargo", async () => {
    vi.mocked(verifyRuntime).mockImplementationOnce(() => {
        throw new Error("Unverified runtime");
    });
    await expect(dependencyNotices([])).rejects.toThrow("Unverified runtime");
    expect(execFileSync).not.toHaveBeenCalled();
});
