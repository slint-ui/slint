// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { Console } from "node:console";
import { Writable } from "node:stream";
import stripAnsi from "strip-ansi";

export function captureLogs() {
    const stdout: string[] = [];
    const stderr: string[] = [];

    const streams = {
        stdout: new Writable({
            write(chunk, _encoding, callback) {
                stdout.push(chunk.toString());
                callback();
            },
        }),
        stderr: new Writable({
            write(chunk, _encoding, callback) {
                stderr.push(chunk.toString());
                callback();
            },
        }),
    };

    const originalConsole = globalThis.console;
    globalThis.console = new Console({
        stdout: streams.stdout,
        stderr: streams.stderr,
    });

    const originalStdoutWrite = process.stdout.write;
    process.stdout.write = streams.stdout.write.bind(streams.stdout) as any;

    const originalStderrWrite = process.stderr.write;
    process.stderr.write = streams.stderr.write.bind(streams.stderr) as any;

    return {
        restore() {
            globalThis.console = originalConsole;
            process.stdout.write = originalStdoutWrite;
            process.stderr.write = originalStderrWrite;
        },
        getLogs() {
            return {
                stdout: stripAnsi(stdout.join("")),
                stderr: stripAnsi(stderr.join("")),
            };
        },
    };
}
