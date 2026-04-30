// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { defineConfig } from "@playwright/test";
import { BASE_PATH } from "../common/src/utils/site-config";
import {
    starlightPlaywrightProjects,
    starlightPlaywrightSharedOptions,
} from "../common/src/testing/playwright-starlight-base";

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
    testDir: "./tests",
    ...starlightPlaywrightSharedOptions(BASE_PATH),
    projects: starlightPlaywrightProjects(),
});
