#!/usr/bin/env node
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

"use strict";

const { spawn } = require("node:child_process");
const { optionalDependencies } = require("./package.json");

function platformTarget() {
    const { platform, arch } = process;
    if (platform === "linux") {
        // The network section of the report can block on hostname resolution.
        process.report.excludeNetwork = true;
        // Only glibc builds report a glibc runtime version.
        const libc = process.report.getReport().header.glibcVersionRuntime
            ? "gnu"
            : "musl";
        return `linux-${arch}-${libc}`;
    }
    if (platform === "win32") {
        return `win32-${arch}-msvc`;
    }
    return `${platform}-${arch}`;
}

const target = platformTarget();
const pkg = `@slint-ui/slint-compiler-${target}`;
const exe =
    process.platform === "win32" ? "slint-compiler.exe" : "slint-compiler";
const fromSource =
    "Build it from source with `cargo install --locked slint-compiler`.";

if (!(pkg in optionalDependencies)) {
    console.error(
        `slint-compiler: there's no prebuilt binary for ${target}.\n${fromSource}`,
    );
    process.exit(1);
}

let binary;
try {
    binary = require.resolve(`${pkg}/${exe}`);
} catch {
    console.error(
        `slint-compiler: ${pkg} isn't installed.\n` +
            `Reinstall without --no-optional or --omit=optional. ${fromSource}`,
    );
    process.exit(1);
}

const child = spawn(binary, process.argv.slice(2), { stdio: "inherit" });
for (const signal of ["SIGINT", "SIGTERM", "SIGHUP"]) {
    process.on(signal, () => child.kill(signal));
}
child.on("error", (error) => {
    console.error(`slint-compiler: failed to run ${binary}: ${error}`);
    process.exit(1);
});
child.on("exit", (code, signal) => {
    if (signal) {
        process.removeAllListeners(signal);
        process.kill(process.pid, signal);
    } else {
        process.exit(code ?? 1);
    }
});
