---
title: Package Your Application
description: Ship a Slint application written in JavaScript or TypeScript as a native executable for Windows, macOS, and Linux.
---

<!-- cSpell: ignore appimagetool codesign entitlements hdiutil icns Inno MSIX myapp notarytool resedit signtool UDZO volname -->

Give your users an application they start like any other,
without installing Node.js or running `npm install`.
`slint-ui pack` builds on Node's [single executable applications](https://nodejs.org/api/single-executable-applications.html):
it makes a copy of the Node.js binary that starts your application,
and puts your application and its production dependencies next to it, as files.

## Build an Executable

Packing runs on Node.js 25.5 or newer, which is also the Node.js your application runs on.
From the directory of your `package.json`:

```sh
npx slint-ui pack
```

That builds the entry point named by `main` in `package.json` into `out/<name>-<platform>-<arch>/`:

| Platform | Result |
|---|---|
| Windows | `<name>.exe`, with your application in `resources` |
| macOS | `<productName>.app`, with the executable in `Contents/MacOS` and your application in `Contents/Resources/app` |
| Linux | `<name>`, with your application in `resources` |

Ship the directory as a whole.

## What Goes Into the Package

- Your package directory, as it is: your code, your `.slint` files, images, fonts, and anything else in it.
  The packer leaves out `node_modules`, the files and directories whose name starts with `.`, and the output directory.
  Keep what your application doesn't need at run time, such as build output or test data, outside that directory,
  or in a directory whose name starts with `.`.
- The packages your application depends on, from `dependencies` and `optionalDependencies` in `package.json`, and the ones they depend on in turn.
  `devDependencies` stay out.
  A package with a native addon, such as `slint-ui`, works the way it does in your project.
- A file or directory outside your package directory that your application reads, such as a directory of `.slint` files it shares with other projects, when you name it with `--asset`:

  ```sh
  npx slint-ui pack --asset ../ui
  ```

The files keep the layout they have in your project, so build the paths between them from `import.meta.url` or `import.meta.dirname`, as in `loadFile(new URL("../ui/main.slint", import.meta.url))`.
A path relative to the working directory, such as `loadFile("ui/main.slint")`, doesn't work:
the working directory is wherever your user starts the application from.

The executable runs your code as Node.js runs it, so `import { MainWindow } from "./main.slint"` works too.
It runs TypeScript directly, with the [type stripping](https://nodejs.org/api/typescript.html) of Node.js.
If your code needs more than that, such as `enum` declarations or path aliases, compile it to JavaScript first, and point `--entry` at the result.

## Options

| Option | Meaning |
|---|---|
| `--entry <file>` | The JavaScript or TypeScript entry point, if it isn't `main` in `package.json`. |
| `--asset <path>` | A file, or a directory of files, outside your package directory that your application reads. |
| `--name <name>` | The name of the executable. The default is `name` in `package.json`. |
| `--product-name <name>` | The name users see. The default is `productName` in `package.json`, then `--name`. |
| `--icon <file>` | The application icon: an `.ico` file for Windows, an `.icns` file for macOS. |
| `--copyright <text>` | The copyright notice shown in the file properties on Windows and in Finder on macOS. |
| `--bundle-id <id>` | The macOS bundle identifier, such as `com.example.myapp`. |
| `--console` | Keep the console window on Windows, which shows the output. |
| `--out <dir>` | Where to write the result. |
| `--platform`, `--arch`, `--node`, `--addon` | Build for another platform. See [Build for Another Platform](#build-for-another-platform). |

The version comes from `version` in `package.json`, the description from `description`, and the company name on Windows from `author`.

## Windows

To build for Windows, install `resedit`, which gives the executable your application's name and icon,
and works on any platform:

```sh
npm install --save-dev resedit
```

The executable starts without a console window, like any other Windows application.
It has nowhere to print to either, even when started from a terminal,
so the output of `console.log()` is lost.
Pass `--console` to keep the console window and see the output, for example while you debug.

Its icon, name, and version come from your application rather than from Node.js,
so Explorer and Task Manager show your application.

The Slint addon needs the Visual C++ runtime.
Install the [Visual C++ redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist) with your application,
or declare it as described in the [Windows packaging guide](https://slint.dev/docs/slint/guide/platforms/desktop/windows/packaging/).

To distribute the result:

- Sign `<name>.exe` and the native addons, which are the `.node` files under `resources`, with `signtool`, after packing.
- Wrap the directory in an installer, such as one made with [Inno Setup](https://jrsoftware.org/isinfo.php) or [WiX](https://wixtoolset.org/),
  or in an MSIX package for the Microsoft Store, as the [Windows packaging guide](https://slint.dev/docs/slint/guide/platforms/desktop/windows/packaging/) describes.

## macOS

The result is an app bundle.
Injecting your application invalidates the signature of the Node.js binary,
so the packer signs the executable again with an ad-hoc signature, which is enough to run it on your own Mac.

Pass `--bundle-id` with an identifier in a domain you own.
Without it, the bundle identifier is `com.example.<name>`.

To distribute the app to other Macs, sign it with your Developer ID and notarize it.
Node.js compiles JavaScript at run time, so the hardened runtime needs to allow it.
Write an `entitlements.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
</dict>
</plist>
```

Then sign the native addons, the `.node` files, then the app, and send the app to Apple:

```sh
APP="out/myapp-darwin-arm64/My App.app"
find "$APP/Contents/Resources" -name "*.node" -exec \
    codesign --force --timestamp --options runtime --sign "Developer ID Application: Your Name" {} \;
codesign --force --timestamp --options runtime --entitlements entitlements.plist \
    --sign "Developer ID Application: Your Name" "$APP"
ditto -c -k --keepParent "$APP" myapp.zip
xcrun notarytool submit myapp.zip --keychain-profile "notary" --wait
xcrun stapler staple "$APP"
```

Finally, put the app in a disk image for your users to download:

```sh
hdiutil create -volname "My App" -srcfolder "$APP" -format UDZO myapp.dmg
```

## Linux

The result runs on distributions with the same or a newer C library than the one Node.js was built against.
Ship the directory in an archive, or in one of the formats Linux desktops install:

- A [Flatpak](https://slint.dev/docs/slint/guide/platforms/desktop/linux/packaging/) that copies the directory in,
  instead of building your application from source.
- An [AppImage](https://appimage.org/), made with `appimagetool` from a directory holding the packed files,
  a `.desktop` file, and an icon.
- A `.deb` or `.rpm` package, which installs the directory under `/opt` and a `.desktop` file under `/usr/share/applications`.

Linux desktops take the application name and icon from the `.desktop` file, so `--icon` doesn't apply.

## Build for Another Platform

You can build for another platform, for example a Windows executable on a Linux build machine.
Pass the target, the Node.js binary for it from the [Node.js downloads](https://nodejs.org/dist/),
and the Slint addon for it.
npm only installs the addon for the platform it runs on,
so fetch the others from npm, in the version of `slint-ui` you use:

```sh
npm pack @slint-ui/slint-ui-binary-win32-x64-msvc@1.19.0
tar xf slint-ui-slint-ui-binary-win32-x64-msvc-1.19.0.tgz
npx slint-ui pack --platform win32 --arch x64 --node ./node-v26.10.0-win-x64/node.exe \
    --addon package/slint-ui.win32-x64-msvc.node
```

A macOS app built elsewhere is unsigned, and macOS refuses to start it.
Sign it on a Mac before you run it, as described in [macOS](#macos).

## Translations

Your `.mo` catalogs are copied like your other files.
Pass `initTranslations()` their directory built from `import.meta.url`, as in `new URL("../lang/", import.meta.url)`,
and name it with `--asset` if it's outside your package directory.

## Limits

- The executable contains Node.js, so a packed application takes over 100 MB.
- A native addon of another package is packed for the platform you build on only,
  so build on each platform your application uses such a package on.
