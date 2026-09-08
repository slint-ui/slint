// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { execFileSync } from "node:child_process";
import { readFile, readdir } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
const runtimeRoot = fileURLToPath(new URL("../../../", import.meta.url));

export async function dependencyNotices(moduleIds) {
    const packages = new Map();
    for (const input of moduleIds) {
        if (!input.includes("node_modules/")) continue;
        let directory = dirname(resolve(input));
        while (directory !== dirname(directory)) {
            try {
                const manifest = JSON.parse(
                    await readFile(resolve(directory, "package.json"), "utf8"),
                );
                if (manifest.name && manifest.version) {
                    packages.set(`npm:${manifest.name}@${manifest.version}`, {
                        name: manifest.name,
                        version: manifest.version,
                        license: manifest.license,
                        directory,
                        ecosystem: "npm",
                    });
                    break;
                }
            } catch {
                /* Continue to the owning package. */
            }
            directory = dirname(directory);
        }
    }
    const metadata = JSON.parse(
        execFileSync(
            "cargo",
            [
                "metadata",
                "--locked",
                "--offline",
                "--format-version",
                "1",
                "--manifest-path",
                resolve(runtimeRoot, "api/wasm-interpreter/Cargo.toml"),
            ],
            { encoding: "utf8", maxBuffer: 100 * 1024 * 1024 },
        ),
    );
    const nodes = new Map(
        metadata.resolve.nodes.map((node) => [node.id, node]),
    );
    const root = metadata.packages.find(
        (pkg) => pkg.name === "slint-wasm-interpreter",
    );
    const visited = new Set();
    const visit = (id) => {
        if (visited.has(id)) return;
        visited.add(id);
        for (const dependency of nodes.get(id)?.deps ?? []) {
            if (dependency.dep_kinds.some((kind) => kind.kind !== "dev"))
                visit(dependency.pkg);
        }
    };
    visit(root.id);
    for (const pkg of metadata.packages.filter((pkg) => visited.has(pkg.id)))
        packages.set(`cargo:${pkg.name}@${pkg.version}`, {
            name: pkg.name,
            version: pkg.version,
            license: pkg.license,
            directory: dirname(pkg.manifest_path),
            licenseFile: pkg.license_file,
            ecosystem: "cargo",
        });
    const sections = [];
    const inventory = [];
    for (const [id, pkg] of [...packages].sort(([a], [b]) =>
        a.localeCompare(b),
    )) {
        const names = (await readdir(pkg.directory)).filter((name) =>
            /^(licen[sc]e|copying|notice)([._-]|$)/i.test(name),
        );
        if (pkg.licenseFile) names.push(pkg.licenseFile);
        const texts = [];
        for (const name of new Set(names)) {
            try {
                texts.push(
                    `${name}\n${await readFile(resolve(pkg.directory, name), "utf8")}`,
                );
            } catch {
                /* License directories are covered by the root license collection. */
            }
        }
        inventory.push({
            id,
            ecosystem: pkg.ecosystem,
            name: pkg.name,
            version: pkg.version,
            license: pkg.license ?? "SEE LICENSE FILE",
            includedLicenseFiles: names,
            hasPackageLicenseText: texts.length > 0,
        });
        sections.push(
            `## ${id}\nLicense: ${pkg.license ?? "SEE LICENSE FILE"}\n\n${texts.join("\n\n")}`,
        );
    }
    // Include upstream license texts, including fonts and vendored components.
    for (const name of (await readdir(resolve(runtimeRoot, "LICENSES"))).sort())
        sections.push(
            `## Slint LICENSES/${name}\n\n${await readFile(resolve(runtimeRoot, "LICENSES", name), "utf8")}`,
        );
    return {
        inventory: {
            scope: "Bundled JavaScript packages and the locked Slint runtime dependency closure, including build and target-specific dependencies. No claim of byte-level Rust tree-shaking attribution.",
            packages: inventory,
        },
        text: `# Third-party notices\n\n${sections.join("\n\n")}`,
    };
}
