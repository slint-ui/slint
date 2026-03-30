// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

/** Map Tabler/astro-icon names to Starlight built-in icon names. */
const TABLER_TO_STARLIGHT: Record<string, string> = {
    "tabler:chevron-down": "down-caret",
    "tabler:sun": "sun",
    "tabler:brand-github": "github",
    "tabler:brand-x": "x.com",
    "tabler:brand-bluesky": "blueSky",
    "tabler:brand-mastodon": "mastodon",
    "tabler:brand-linkedin": "linkedin",
    "tabler:brand-youtube": "youtube",
    "tabler:chevron-right": "right-arrow",
    "tabler:check": "approve-check-circle",
    "tabler:square-rounded-arrow-right": "right-arrow",
    "tabler:award": "star",
    "tabler:gauge": "rocket",
    "tabler:play-card-10": "document",
    "tabler:devices": "vscode",
    "tabler:hand-click": "approve-check-circle",
    "tabler:paint": "figma",
    "tabler:download": "down-arrow",
    "tabler:browser": "document",
};

/**
 * Returns the Starlight icon name for a given icon (e.g. tabler:foo or already a Starlight name).
 */
export function toStarlightIconName(name: string): string {
    return TABLER_TO_STARLIGHT[name] ?? name;
}
