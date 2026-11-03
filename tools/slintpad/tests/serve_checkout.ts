// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell:ignore githubusercontent

import type { Page } from "@playwright/test";
import { readFile } from "node:fs/promises";
import path from "node:path";

// Playwright runs from tools/slintpad; the repository root is two levels up.
const REPO_ROOT = path.resolve(process.cwd(), "..", "..");

/// Answers requests for raw.githubusercontent.com/slint-ui/slint/<ref>/<file>
/// with <file> from this checkout, after `latency_ms`,
/// so a test is hermetic and pinned to the current sources.
export async function serve_checkout(
    page: Page,
    latency_ms = 0,
): Promise<void> {
    await page.route(
        /^https:\/\/raw\.githubusercontent\.com\/slint-ui\/slint\/[^/]+\//,
        async (route) => {
            const rel = decodeURIComponent(
                new URL(route.request().url()).pathname.replace(
                    /^\/slint-ui\/slint\/[^/]+\//,
                    "",
                ),
            );
            if (latency_ms > 0) {
                await new Promise((resolve) => setTimeout(resolve, latency_ms));
            }
            try {
                await route.fulfill({
                    status: 200,
                    body: await readFile(path.join(REPO_ROOT, rel)),
                });
            } catch {
                await route.fulfill({ status: 404, body: "" });
            }
        },
    );
}
