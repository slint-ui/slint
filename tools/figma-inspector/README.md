<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

## Figma to Slint property inspector

### Installing the plugin from Figma

The latest release of the Figma Inspector can be installed directly from the Figma website at

    https://www.figma.com/community/plugin/1474418299182276871/figma-to-slint

or in Figma by searching for the "Figma To Slint" plugin.

### Installing the plugin via nightly snapshot.

Download the nightly snapshot [figma-plugin.zip](https://github.com/slint-ui/slint/releases/download/nightly/figma-plugin.zip).

The prerequisites are either the Figma Desktop App or the Figma VSCode extension.
A valid Figma subscription with at least 'Team Professional' is needed.

In Figma Desktop or the VScode extension have a file open and right click on it. Select Plugins > Development > Import Plugin From Manifest.. and point it at the manifest.json file that you just unzipped.

The Slint properties will now show in the Dev mode inspector in the same place the standard CSS properties
would have been shown.

### Build

Figma is a web app (Chromium) and the plugin is just javascript. As with other web apps in the repo
the prerequisite software needed to develop the plugin are:

You need to install the following components:
* **[Node.js](https://nodejs.org/download/release/)** (v20. or newer)
* **[pnpm](https://www.pnpm.io/)**
* **[Figma Desktop App](https://www.figma.com/downloads/)**

You also **MUST** have a valid Figma developer subscription as the plugin works in the Dev mode
and/or Figma VS Code extension.

To try it out locally type this in this directory:

```sh
## only need to run this once
pnpm install

pnpm build
```

Then in Figma on an open file right click and select `Plugins > Development > Import Plugin From Manifest..` and point it at the `dist/manifest.json` file that has now been created inside this project.

You should also ensure `Plugins > Development > Hot Reload Plugin` is ticked.

To develop in hot reload mode:

```sh
pnpm dev
```

As you save code changes the plugin is automatically recompiled and reloaded in Figma for Desktop and/or the Figma VS Code extension.


### Testing

As of writing Figma has real test support. Testing is limited to unit testing some of the functions via `Vitest`.

You can find the test files under `/tests`. This folder also includes the JSON export of a real Figma file
to test against. The easiest way to update the file is to to edit it in Figma and then use a personal access token to get a JSON version.

To get an access Token in Figma go to the home screen. Then top right click the logged in user name. Then `Settings` and then the `Security` tab. Scroll to the bottom and choose `Generate new token`. Then save the token in a secure private place.

You then need to get the file ID. Open figma.com, login and open the file. You will then have a url like
`https://www.figma.com/design/njC6jSUbrYpqLRJ2dyV6NT/energy-test-file?node-id=113-2294&p=f&t=5IDwrGIFUnri3Z17-0`. The ID is the part of the URL after `/design/` so in this example `njC6jSUbrYpqLRJ2dyV6NT`.

You can then use `curl` to download the JSON with

```sh
curl -H 'X-Figma-Token: <YOUR_ACCESS_TOKEN>' \
'https://api.figma.com/v1/files/<FIGMA_FILE_ID>' \
-o figma_output.json
```

Vitest can then be run in hot reload mode for ease of test development with:

```sh
pnpm test
```


