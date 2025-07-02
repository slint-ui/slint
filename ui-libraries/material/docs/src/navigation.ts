// Copyright © onWidget <https://github.com/onwidget>
// SPDX-License-Identifier: MIT
import { getPermalink, getAsset } from "./utils/permalinks";

export const headerData = {
    links: [
        {
            text: "Documentation",
            href: "/getting-started/",
        },
        {
            text: "Demo",
            href: "/wasm/index.html",
        },
    ],
};

export const footerData = {
    links: [
        {
            title: "Company",
            links: [
                { text: "Learn more about Slint", href: "https://slint.dev/" },
                { text: "Privacy Policy", href: "https://slint.dev/imprint" },
            ],
        },
    ],
    secondaryLinks: [],
    socialLinks: [
        {
            ariaLabel: "Github",
            icon: "tabler:brand-github",
            href: "https://github.com/slint-ui/material-components",
        },
        {
            ariaLabel: "X",
            icon: "tabler:brand-x",
            href: "https://x.com/slint_ui",
        },
        {
            ariaLabel: "Bluesky",
            icon: "tabler:brand-bluesky",
            href: "https://bsky.app/profile/slint.dev",
        },
        {
            ariaLabel: "Mastodon",
            icon: "tabler:brand-mastodon",
            href: "https://fosstodon.org/@slint",
        },
        {
            ariaLabel: "LinkedIn",
            icon: "tabler:brand-linkedin",
            href: "https://www.linkedin.com/company/slint-ui",
        },
        {
            ariaLabel: "YouTube",
            icon: "tabler:brand-youtube",
            href: "https://www.youtube.com/@slint-ui",
        },
    ],
    footNote: `
    Copyright © 2025 SixtyFPS GmbH
  `,
};
