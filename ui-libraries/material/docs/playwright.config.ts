// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { defineConfig } from "@playwright/test";
import {
    starlightPlaywrightProjects,
    starlightPlaywrightSharedOptions,
} from "../../../docs/common/src/testing/playwright-starlight-base.ts";

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
    testDir: "./tests",
    ...starlightPlaywrightSharedOptions(
        process.env.MATERIAL_DOCS_BASE_PATH || "/",
    ),
    projects: starlightPlaywrightProjects(),
});
