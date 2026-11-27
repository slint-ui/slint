// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cspell:ignore gdbus HKCU

import { execFile } from "node:child_process";
import { promisify } from "node:util";

const run = promisify(execFile);
const options = { timeout: 500, windowsHide: true };

export async function systemColorScheme() {
    try {
        switch (process.platform) {
            case "darwin": {
                const { stdout } = await run(
                    "/usr/bin/defaults",
                    ["read", "-g", "AppleInterfaceStyle"],
                    options,
                );
                // macOS typically leaves the key unset in light mode.
                return stdout.trim() === "Dark" ? "dark" : undefined;
            }
            case "win32": {
                const key =
                    "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
                const { stdout } = await run(
                    "reg.exe",
                    ["query", key, "/v", "AppsUseLightTheme"],
                    options,
                );
                const value = stdout.match(
                    /^\s*AppsUseLightTheme\s+REG_DWORD\s+0x([01])\s*$/im,
                )?.[1];
                return { 0: "dark", 1: "light" }[value];
            }
            case "linux": {
                // ReadAll is available on older portals that lack ReadOne.
                const { stdout } = await run(
                    "gdbus",
                    [
                        "call",
                        "--session",
                        "--dest",
                        "org.freedesktop.portal.Desktop",
                        "--object-path",
                        "/org/freedesktop/portal/desktop",
                        "--method",
                        "org.freedesktop.portal.Settings.ReadAll",
                        "['org.freedesktop.appearance']",
                    ],
                    options,
                );
                const value = stdout.match(
                    /'color-scheme': <uint32 ([12])>/,
                )?.[1];
                return { 1: "dark", 2: "light" }[value];
            }
        }
    } catch {
        // Headless or remote sessions may lack access to desktop settings.
        return undefined;
    }
}
