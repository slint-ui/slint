// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import { defineConfig } from "@playwright/test";
import { MATERIAL_DOCS_BASE_PATH } from "../../../docs/common/src/utils/site-config.ts";
import {
    starlightPlaywrightProjects,
    starlightPlaywrightSharedOptions,
} from "../../../docs/common/src/testing/playwright-starlight-base.ts";

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
    testDir: "./tests",
    ...starlightPlaywrightSharedOptions(MATERIAL_DOCS_BASE_PATH),
    projects: starlightPlaywrightProjects(),
});
