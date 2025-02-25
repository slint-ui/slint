<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

## Publishing The Plugin

The official Figma store has a manual publishing process that cannot be automated.


### Prerequisites

1. Have a valid Figma Account AND have 2 factor authentication enabled. Plugin cannot be submitted without 2FA.
2. Have an admin account under the SixtyFPS GmbH figma team.



1. Build the plugin as per the README.
2. Open Figma for Desktop and login with your @slint.dev account.
3. On the left sidebar ensure the team is set to "SixtyFPS GmbH" it most likely
defaulted to your personal team, not this one.
4. Open the plugin in Figma for Desktop via the `dist/manifest.json` file.
5. In Figma for Desktop select from the menu Plugins -> Manage Plugins...
6. On the right of `Figma to Slint` is a menu (3 dots) and chose publish.
7. Ensure all fields are filled in and the support contact is `info@slint.dev`.
8. Ensure the publisher shows as SixtyFPS GmbH and not your personal account. If the drop down is missing
SixtyFPS GmbH see step 3.
9. Publish.