// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { devices, type PlaywrightTestConfig } from "@playwright/test";

/**
 * Default browser projects for Slint Starlight Playwright suites (main docs, Material, etc.).
 */
export function starlightPlaywrightProjects(): PlaywrightTestConfig["projects"] {
    return [
        { name: "chromium", use: { ...devices["Desktop Chrome"] } },
        { name: "firefox", use: { ...devices["Desktop Firefox"] } },
        { name: "webkit", use: { ...devices["Desktop Safari"] } },
    ];
}

/**
 * Shared Starlight docs test options: reporters, preview webServer, trace, parallel settings.
 *
 * @param basePath - Astro `base` path prefix (e.g. {@link BASE_PATH} or {@link MATERIAL_DOCS_BASE_PATH}).
 */
export function starlightPlaywrightSharedOptions(
    basePath: string,
): Pick<
    PlaywrightTestConfig,
    | "fullyParallel"
    | "forbidOnly"
    | "retries"
    | "workers"
    | "reporter"
    | "use"
    | "webServer"
> {
    return {
        fullyParallel: true,
        forbidOnly: !!process.env.CI,
        retries: process.env.CI ? 2 : 0,
        workers: process.env.CI ? 1 : undefined,
        reporter: [
            ["html"],
            [
                "playwright-ctrf-json-reporter",
                {
                    outputFile: "ctrf-report.json",
                    outputDir: "playwright-report",
                },
            ],
        ],
        use: {
            baseURL: `http://localhost:4321${basePath}`,
            trace: "on-first-retry",
        },
        webServer: {
            command: "pnpm run preview",
            url: `http://localhost:4321${basePath}`,
            reuseExistingServer: !process.env.CI,
            timeout: 120 * 1000,
        },
    };
}
