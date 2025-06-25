// Copyright © onWidget <https://github.com/onwidget>
// SPDX-License-Identifier: MIT
import { getPermalink, getAsset } from "./utils/permalinks";

export const headerData = {
    links: [
        {
            text: "Documentation",
            href: "/overview/",
        },
        {
            text: "Demo",
            href: "/wasm/index.html",
        },
    ],
};

export const footerData = {
    links: [
        // {
        //   title: 'Product',
        //   links: [
        //     { text: 'Features', href: '#' },
        //     { text: 'Security', href: '#' },
        //     { text: 'Team', href: '#' },
        //     { text: 'Enterprise', href: '#' },
        //     { text: 'Customer stories', href: '#' },
        //     { text: 'Pricing', href: '#' },
        //     { text: 'Resources', href: '#' },
        //   ],
        // },
        // {
        //   title: 'Platform',
        //   links: [
        //     { text: 'Developer API', href: '#' },
        //     { text: 'Partners', href: '#' },
        //     { text: 'Atom', href: '#' },
        //     { text: 'Electron', href: '#' },
        //     { text: 'AstroWind Desktop', href: '#' },
        //   ],
        // },
        // {
        //   title: 'Support',
        //   links: [
        //     { text: 'Docs', href: '#' },
        //     { text: 'Community Forum', href: '#' },
        //     { text: 'Professional Services', href: '#' },
        //     { text: 'Skills', href: '#' },
        //     { text: 'Status', href: '#' },
        //   ],
        // },
        // {
        //   title: 'Company',
        //   links: [
        //     { text: 'About', href: '#' },
        //     { text: 'Blog', href: '#' },
        //     { text: 'Careers', href: '#' },
        //     { text: 'Press', href: '#' },
        //     { text: 'Inclusion', href: '#' },
        //     { text: 'Social Impact', href: '#' },
        //     { text: 'Shop', href: '#' },
        //   ],
        // },
    ],
    secondaryLinks: [
        { text: "Privacy Policy", href: getPermalink("/privacy") },
    ],
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
