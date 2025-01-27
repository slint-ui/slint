<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0 -->

## Figma to Slint property inspector

### Features
- UI that follows Figma theme colors and light/dark mode.
- Only displays properties for a single selected item. Warns if nothing or multiple items are selected.
- Displayed properties are re-written to fit Slint syntax.
- Copy to clipboard button.
- Don't show copy if there are no properties.

### Future tweaks
- Set plugin up to build via Vite. Figma only works with a single *.js file. Using Vite means we
can have multiple files, import libraries, use react, etc and it will sort out a bundle that just becomes one *.js file.
- Can Shiki be used to syntax color the properties?
- Still some missing properties and thought needed for how to deal with Figma properties that don't exist in Slint e.g. you can have unlimited individual shadows and gradients. Slint only supports one of each.
- Grab the 'text' value for any text from the original data.


Below are the steps to get your plugin running. You can also find instructions at:

  https://www.figma.com/plugin-docs/plugin-quickstart-guide/

Enusure you have the desktop version of Figma installed.

First install the dependencies with `pnpm i`.
Then build the project with `pnpm build`.
Then in Figma select Plugins > Plugins & Widgets > Import from manifest... and then chose the manifest.json from this folder.

## Sending Messages between the Frontend and Backend

Bolt Figma makes messaging between the frontend UI and backend code layers simple and type-safe. This can be done with `listenTS()` and `dispatchTS()`.

Using this method accounts for:

- Setting up a scoped event listener in the listening context
- Removing the listener when the event is called (if `once` is set to true)
- Ensuring End-to-End Type-Safety for the event

### 1. Declare the Event Type in EventTS in shared/universals.ts

```js
export type EventTS = {
  myCustomEvent: {
    oneValue: string,
    anotherValue: number,
  },
  // [... other events]
};
```

### 2a. Send a Message from the Frontend to the Backend

**Backend Listener:** `src-code/code.ts`

```js
import { listenTS } from "./utils/code-utils";

listenTS("myCustomEvent", (data) => {
  console.log("oneValue is", data.oneValue);
  console.log("anotherValue is", data.anotherValue);
});
```

**Frontend Dispatcher:** `index.svelte` or `index.tsx` or `index.vue`

```js
import { dispatchTS } from "./utils/utils";

dispatchTS("myCustomEvent", { oneValue: "name", anotherValue: 20 });
```

### 2b. Send a Message from the Backend to the Frontend

**Frontend Listener:** `index.svelte` or `index.tsx` or `index.vue`

```js
import { listenTS } from "./utils/utils";

listenTS(
  "myCustomEvent",
  (data) => {
    console.log("oneValue is", data.oneValue);
    console.log("anotherValue is", data.anotherValue);
  },
  true,
);
```

_Note: `true` is passed as the 3rd argument which means the listener will only listen once and then be removed. Set this to true to avoid duplicate events if you only intend to recieve one reponse per function._

**Backend Dispatcher:** `src-code/code.ts`

```js
import { dispatchTS } from "./utils/code-utils";

dispatchTS("myCustomEvent", { oneValue: "name", anotherValue: 20 });
```

---

### Info on Build Process

Frontend code is built to the `.tmp` directory temporarily and then copied to the `dist` folder for final. This is done to avoid Figma throwing plugin errors with editing files directly in the `dist` folder.

The frontend code (JS, CSS, HTML) is bundled into a single `index.html` file and all assets are inlined.

The backend code is bundled into a single `code.js` file.

Finally the `manifest.json` is generated from the `figma.config.ts` file with type-safety. This is configured when running `yarn create bolt-figma`, but you can make additional modifications to the `figma.config.ts` file after initialization.

### Read if Dev or Production Mode

Use the built-in Vite env var MODE to determine this:

```js
const mode = import.meta.env.MODE; // 'dev' or 'production'
```

### Troubleshooting Assets

Figma requires the entire frontend code to be wrapped into a single HTML file. For this reason, bundling external images, svgs, and other assets is not possible.

The solution to this is to inline all assets. Vite is already setup to inline most asset types it understands such as JPG, PNG, SVG, and more, however if the file type you're trying to inline doesn't work, you may need to add it to the assetsInclude array in the vite config:

More Info: https://vitejs.dev/config/shared-options.html#assetsinclude

Additionally, you may be able to import the file as a raw string, and then use that data inline in your component using the `?raw` suffix.

For example:

```ts
import icon from "./assets/icon.svg?raw";
```

and then use that data inline in your component:

```js
// Svelte
{@html icon}

// React
<div dangerouslySetInnerHTML={{ __html: icon }}></div>

// Vue
<div v-html="icon"></div>
```


