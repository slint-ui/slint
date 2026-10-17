// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { AstroUserConfig } from "astro";
import { defineConfig, mergeConfig } from "astro/config";

export function defineSlintDocsConfig(config: AstroUserConfig) {
    return defineConfig(
        mergeConfig(config, {
            vite: {
                environments: {
                    prerender: {
                        resolve: {
                            noExternal: ["js-yaml"],
                        },
                    },
                },
            },
        }),
    );
}
