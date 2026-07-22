// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT
import type { StarlightUserConfig } from "@astrojs/starlight/types";

type StarlightSocial = NonNullable<StarlightUserConfig["social"]>;

const SLINT_GITHUB_HREF = "https://github.com/slint-ui/slint";

/**
 * Canonical Starlight `social` links for all Slint doc sites (Material lives in the same repo).
 */
export const slintStarlightSocial: StarlightSocial = [
    { icon: "github", label: "GitHub", href: SLINT_GITHUB_HREF },
    { icon: "x.com", label: "X", href: "https://x.com/slint_ui" },
    {
        icon: "blueSky",
        label: "Bluesky",
        href: "https://bsky.app/profile/slint.dev",
    },
    {
        icon: "linkedin",
        label: "Linkedin",
        href: "https://www.linkedin.com/company/slint-ui/",
    },
    {
        icon: "mastodon",
        label: "Mastodon",
        href: "https://fosstodon.org/@slint",
    },
    {
        icon: "youtube",
        label: "YouTube",
        href: "https://www.youtube.com/@slint-ui",
    },
] as StarlightSocial;
