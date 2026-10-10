// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore APPL

// `slint-ui pack`: turn an application into a directory with a Node.js single executable application.
// See docs/nodejs/src/content/docs/packaging.md for the user-facing side.

import * as fs from "node:fs";
import * as path from "node:path";
import { execFileSync } from "node:child_process";
import { createRequire } from "node:module";
import { parseArgs } from "node:util";

import type * as reseditTypes from "resedit";

const USAGE = `Usage: slint-ui pack [options]

Package the application in the current directory, with its production dependencies,
next to a Node.js executable.

Options:
  --entry <file>         JavaScript or TypeScript entry point (default: "main" in package.json)
  --asset <path>         A file, or a directory of files, outside the package that the
                         application reads. Repeatable.
  --name <name>          Executable name (default: "name" in package.json)
  --product-name <name>  Name shown to users (default: "productName" in package.json, or --name)
  --icon <file>          Application icon: .ico for Windows, .icns for macOS
  --copyright <text>     Copyright notice for Windows and macOS
  --out <dir>            Output directory (default: out/<name>-<platform>-<arch>)
  --platform <platform>  Target platform: linux, darwin, or win32 (default: this one)
  --arch <arch>          Target architecture: x64 or arm64 (default: this one)
  --node <file>          The Node.js binary of the target (default: this one)
  --addon <file>         The Slint addon of the target, when building for another platform
  --bundle-id <id>       macOS bundle identifier, such as com.example.app
  --console              On Windows, keep the console window, which shows the output
  -h, --help             Show this help
`;

/** The file next to the application's package that the executable starts. */
const START_FILE = ".slint-pack-start.cjs";

class UsageError extends Error {}

interface Options {
    app: string;
    entry: string;
    assets: string[];
    name: string;
    productName: string;
    version: string;
    description: string | undefined;
    author: string | undefined;
    copyright: string | undefined;
    icon: string | undefined;
    out: string;
    platform: NodeJS.Platform;
    arch: string;
    node: string;
    addon: string | undefined;
    bundleId: string | undefined;
    /** Start without a console window, as Windows GUI applications do. */
    windowsGui: boolean;
}

function parseOptions(argv: string[]): Options {
    const { values } = parseArgs({
        args: argv,
        options: {
            entry: { type: "string" },
            asset: { type: "string", multiple: true, default: [] },
            name: { type: "string" },
            "product-name": { type: "string" },
            icon: { type: "string" },
            copyright: { type: "string" },
            out: { type: "string" },
            platform: { type: "string", default: process.platform },
            arch: { type: "string", default: process.arch },
            node: { type: "string" },
            addon: { type: "string" },
            "bundle-id": { type: "string" },
            console: { type: "boolean", default: false },
            help: { type: "boolean", short: "h", default: false },
        },
    });
    if (values.help) {
        process.stdout.write(USAGE);
        process.exit(0);
    }

    const app = process.cwd();
    const packageJson = readPackageJson(app);
    const entry = values.entry ?? packageJson.main;
    if (typeof entry !== "string") {
        throw new UsageError(
            'no entry point: set "main" in package.json, or pass --entry',
        );
    }
    const name =
        values.name ?? String(packageJson.name ?? "app").replace(/^@.*\//, "");
    const platform = values.platform as NodeJS.Platform;
    if (!["linux", "darwin", "win32"].includes(platform)) {
        throw new UsageError(`unsupported platform "${platform}"`);
    }
    const arch = values.arch;
    const cross = platform !== process.platform || arch !== process.arch;
    if (cross && (values.node === undefined || values.addon === undefined)) {
        throw new UsageError(
            `building for ${platform}-${arch} on ${process.platform}-${process.arch} needs the target's Node.js binary and Slint addon: pass --node and --addon`,
        );
    }

    const icon = values.icon ? path.resolve(app, values.icon) : undefined;
    const iconExtension = { win32: ".ico", darwin: ".icns" }[
        platform as string
    ];
    if (icon && iconExtension && path.extname(icon) !== iconExtension) {
        throw new UsageError(
            `--icon for ${platform} takes a ${iconExtension} file`,
        );
    }
    const author = packageJson.author;

    return {
        app,
        entry: path.resolve(app, entry),
        assets: values.asset.map((file) => path.resolve(app, file)),
        name,
        productName: values["product-name"] ?? packageJson.productName ?? name,
        version: String(packageJson.version ?? "0.0.0"),
        description: packageJson.description,
        author:
            typeof author === "string"
                ? author.replace(/\s*[<(].*$/, "")
                : author?.name,
        copyright: values.copyright,
        icon,
        out: path.resolve(
            app,
            values.out ?? path.join("out", `${name}-${platform}-${arch}`),
        ),
        platform,
        arch,
        node: path.resolve(values.node ?? process.execPath),
        addon: values.addon && path.resolve(values.addon),
        bundleId: values["bundle-id"],
        windowsGui: platform === "win32" && !values.console,
    };
}

function readPackageJson(dir: string) {
    return JSON.parse(fs.readFileSync(path.join(dir, "package.json"), "utf8"));
}

/** The name napi-rs gives the addon for a target, such as `linux-x64-gnu`. */
function addonTarget(platform: string, arch: string): string {
    switch (platform) {
        case "linux":
            return `linux-${arch}-gnu`;
        case "win32":
            return `win32-${arch}-msvc`;
        default:
            return `${platform}-${arch}`;
    }
}

/** The module `id`, installed by the application or next to `slint-ui`. */
function requireOptional(app: string, id: string, needed: string) {
    for (const base of [path.join(app, "package.json"), __filename]) {
        try {
            return createRequire(base)(id);
        } catch {}
    }
    throw new UsageError(`slint-ui pack needs ${needed}`);
}

/** Where the application's files go, relative to the directory of the executable. */
function resourcesFromExecutable(platform: NodeJS.Platform): string {
    return platform === "darwin" ? "../Resources/app" : "resources";
}

/** The directory of the package `name` that code in `dir` imports, as Node.js finds it. */
function findPackage(name: string, dir: string): string | undefined {
    for (let at = dir; ; at = path.dirname(at)) {
        const candidate = path.join(at, "node_modules", name);
        if (fs.existsSync(path.join(candidate, "package.json"))) {
            return fs.realpathSync(candidate);
        }
        if (path.dirname(at) === at) {
            return undefined;
        }
    }
}

/**
 * Lay out the production dependencies of the package in `dir` under `into`,
 * as `node_modules` directories Node.js resolves them from, and return their files.
 *
 * Each package goes at the top level unless another version is already there,
 * in which case it goes in the `node_modules` of the package that needs it.
 */
function dependencies(dir: string, into: string, options: Options) {
    const placed = new Map<string, string>();
    const files: [string, string][] = [];
    const place = (from: string, parent: string) => {
        const packageJson = readPackageJson(from);
        const wanted = [
            ...Object.keys(packageJson.dependencies ?? {}),
            ...Object.keys(packageJson.optionalDependencies ?? {}),
        ];
        for (const name of wanted) {
            const found = findPackage(name, from);
            if (found === undefined) {
                continue; // An optional dependency that isn't installed.
            }
            const { os, cpu } = readPackageJson(found);
            if (
                (os && !os.includes(options.platform)) ||
                (cpu && !cpu.includes(options.arch))
            ) {
                continue; // A package for another platform.
            }
            const top = path.join(into, "node_modules", name);
            const target =
                placed.get(top) === undefined || placed.get(top) === found
                    ? top
                    : path.join(parent, "node_modules", name);
            if (placed.get(target) === found) {
                continue;
            }
            placed.set(target, found);
            files.push(...copyList(found, target));
            place(found, target);
        }
    };
    place(dir, into);
    return files;
}

/** The files of the package in `dir`, as [source, destination] pairs under `into`. */
function copyList(
    dir: string,
    into: string,
    skip: string[] = [],
): [string, string][] {
    return listFiles(dir)
        .filter((file) => {
            const parts = path.relative(dir, file).split(path.sep);
            return (
                !parts.includes("node_modules") &&
                !parts[0].startsWith(".") &&
                !skip.some((skipped) => file.startsWith(skipped + path.sep))
            );
        })
        .map((file) => [file, path.join(into, path.relative(dir, file))]);
}

/** The files in `dir` and its subdirectories, including the targets of symbolic links. */
function listFiles(dir: string): string[] {
    return fs
        .readdirSync(dir, { recursive: true, withFileTypes: true })
        .map((entry) => path.join(entry.parentPath, entry.name))
        .filter((file) => fs.statSync(file).isFile());
}

/** The files under each of `paths`, which name files or directories. */
function filesUnder(paths: string[]): string[] {
    return paths.flatMap((file) =>
        fs.statSync(file).isDirectory() ? listFiles(file) : [file],
    );
}

/** The deepest directory that contains all of `paths`. */
function commonAncestor(paths: string[]): string {
    return paths.reduce((ancestor, file) => {
        while (path.relative(ancestor, file).startsWith("..")) {
            ancestor = path.dirname(ancestor);
        }
        return ancestor;
    });
}

/** The numeric parts of a version such as `1.2.3-beta`, the form Windows and macOS store. */
function numericVersion(version: string): [number, number, number] {
    const [major = 0, minor = 0, patch = 0] = version
        .split(/[.+-]/, 3)
        .map((part) => Number.parseInt(part, 10) || 0);
    return [major, minor, patch];
}

/**
 * Copy `source` to `target`, with the application's icon and version information instead of
 * those of Node.js, and as a GUI application if it is one.
 */
async function setWindowsResources(
    options: Options,
    source: string,
    target: string,
) {
    const resedit: typeof reseditTypes = await requireOptional(
        options.app,
        "resedit/cjs",
        "resedit for a Windows executable: npm install --save-dev resedit",
    ).load();
    const executable = resedit.NtExecutable.from(fs.readFileSync(source), {
        ignoreCert: true,
    });
    if (options.windowsGui) {
        const IMAGE_SUBSYSTEM_WINDOWS_GUI = 2;
        executable.newHeader.optionalHeader.subsystem =
            IMAGE_SUBSYSTEM_WINDOWS_GUI;
    }
    const resources = resedit.NtExecutableResource.from(executable);

    if (options.icon) {
        const icons = resedit.Data.IconFile.from(
            fs.readFileSync(options.icon),
        ).icons.map((icon) => icon.data);
        for (const group of resedit.Resource.IconGroupEntry.fromEntries(
            resources.entries,
        )) {
            resedit.Resource.IconGroupEntry.replaceIconsForResource(
                resources.entries,
                group.id,
                group.lang,
                icons,
            );
        }
    }

    const version = [...numericVersion(options.version), 0] as const;
    for (const info of resedit.Resource.VersionInfo.fromEntries(
        resources.entries,
    )) {
        for (const language of info.getAllLanguagesForStringValues()) {
            for (const key of ["CompanyName", "LegalCopyright", "Comments"]) {
                info.removeStringValue(language, key);
            }
            const values: Record<string, string> = {
                ProductName: options.productName,
                FileDescription: options.description ?? options.productName,
                FileVersion: options.version,
                ProductVersion: options.version,
                OriginalFilename: `${options.name}.exe`,
                InternalName: options.name,
            };
            if (options.author) {
                values.CompanyName = options.author;
            }
            if (options.copyright) {
                values.LegalCopyright = options.copyright;
            }
            // After the numeric versions, which overwrite the version strings.
            info.setFileVersion(...version, language.lang);
            info.setProductVersion(...version, language.lang);
            info.setStringValues(language, values);
        }
        info.outputToResourceEntries(resources.entries);
    }

    resources.outputResource(executable);
    fs.writeFileSync(target, Buffer.from(executable.generate()));
}

function infoPlist(options: Options): string {
    const version = numericVersion(options.version).join(".");
    const entries: Record<string, string | boolean> = {
        CFBundleDevelopmentRegion: "en",
        CFBundleDisplayName: options.productName,
        CFBundleExecutable: options.name,
        CFBundleIdentifier: options.bundleId ?? `com.example.${options.name}`,
        CFBundleInfoDictionaryVersion: "6.0",
        CFBundleName: options.productName,
        CFBundlePackageType: "APPL",
        CFBundleShortVersionString: version,
        CFBundleVersion: version,
        NSHighResolutionCapable: true,
    };
    if (options.icon) {
        entries.CFBundleIconFile = path.basename(options.icon);
    }
    if (options.copyright) {
        entries.NSHumanReadableCopyright = options.copyright;
    }
    const escape = (s: string) =>
        s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
    const body = Object.entries(entries)
        .map(
            ([key, value]) =>
                `    <key>${key}</key>\n    ${typeof value === "boolean" ? `<${value}/>` : `<string>${escape(value)}</string>`}`,
        )
        .join("\n");
    return `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
${body}
</dict>
</plist>
`;
}

async function pack(options: Options) {
    fs.mkdirSync(options.out, { recursive: true });
    const work = fs.mkdtempSync(path.join(options.out, ".build-"));
    try {
        await packIn(work, options);
    } finally {
        fs.rmSync(work, { recursive: true, force: true });
    }
}

/** Write the app bundle around the executable, and return the directory of the executable. */
function writeMacBundle(options: Options): string {
    const appBundle = path.join(options.out, `${options.productName}.app`);
    fs.rmSync(appBundle, { recursive: true, force: true });
    const contents = path.join(appBundle, "Contents");
    fs.mkdirSync(path.join(contents, "MacOS"), { recursive: true });
    fs.mkdirSync(path.join(contents, "Resources"), { recursive: true });
    fs.writeFileSync(path.join(contents, "Info.plist"), infoPlist(options));
    if (options.icon) {
        fs.copyFileSync(
            options.icon,
            path.join(contents, "Resources", path.basename(options.icon)),
        );
    }
    return path.join(contents, "MacOS");
}

async function packIn(work: string, options: Options) {
    const base = commonAncestor([options.app, ...options.assets]);
    const binDir =
        options.platform === "darwin" ? writeMacBundle(options) : options.out;
    const resources = path.join(
        binDir,
        resourcesFromExecutable(options.platform),
    );
    const app = path.join(resources, path.relative(base, options.app));

    const files = [
        ...copyList(options.app, app, [options.out]),
        ...dependencies(options.app, app, options),
        ...filesUnder(options.assets).map((file): [string, string] => [
            file,
            path.join(resources, path.relative(base, file)),
        ]),
    ];
    fs.rmSync(resources, { recursive: true, force: true });
    for (const [from, to] of files) {
        fs.mkdirSync(path.dirname(to), { recursive: true });
        fs.copyFileSync(from, to);
    }
    if (options.addon) {
        const slintDir = path.join(app, "node_modules", "slint-ui");
        const target = addonTarget(options.platform, options.arch);
        fs.copyFileSync(
            options.addon,
            path.join(slintDir, `slint-ui.${target}.node`),
        );
    }

    // The executable can only load built-in modules itself, and a file it requires
    // can't use top-level await, so it requires a file that imports the entry.
    const entry = JSON.stringify(
        "./" +
            path.relative(options.app, options.entry).split(path.sep).join("/"),
    );
    fs.writeFileSync(
        path.join(app, START_FILE),
        `import("slint-ui/register").then(() => import(${entry}));\n`,
    );
    const main = path.join(work, "main.cjs");
    const appFromExecutable = path
        .relative(binDir, app)
        .split(path.sep)
        .join("/");
    fs.writeFileSync(
        main,
        `const path = require("node:path");
const { createRequire } = require("node:module");
const start = path.join(path.dirname(process.execPath), ${JSON.stringify(appFromExecutable)}, ${JSON.stringify(START_FILE)});
createRequire(start)(start);
`,
    );

    const executable = path.join(
        binDir,
        options.platform === "win32" ? `${options.name}.exe` : options.name,
    );
    let node = options.node;
    if (options.platform === "win32") {
        node = path.join(work, "node.exe");
        await setWindowsResources(options, options.node, node);
    }

    const config = path.join(work, "sea-config.json");
    fs.writeFileSync(
        config,
        JSON.stringify({
            main,
            executable: node,
            output: executable,
            disableExperimentalSEAWarning: true,
        }),
    );
    execFileSync(process.execPath, ["--build-sea", config], {
        stdio: ["ignore", "ignore", "inherit"],
    });

    console.log(
        `Packed ${path.relative(options.app, options.entry)} and ${files.length} files into ${executable}`,
    );
    if (options.platform === "darwin") {
        if (options.bundleId === undefined) {
            console.log(
                `Note: The bundle identifier is com.example.${options.name}: pass --bundle-id with your own.`,
            );
        }
        // Injecting the application invalidated the signature of the Node.js binary.
        if (process.platform === "darwin") {
            execFileSync("codesign", ["--sign", "-", "--force", executable]);
        } else {
            console.log(
                `Note: Sign the app on macOS before running it: codesign --sign - --force "${executable}"`,
            );
        }
    }
}

function nodeIsAtLeast(major: number, minor: number): boolean {
    const [actualMajor, actualMinor] = process.versions.node
        .split(".")
        .map(Number);
    return (
        actualMajor > major || (actualMajor === major && actualMinor >= minor)
    );
}

export async function main(argv: string[]) {
    try {
        if (!nodeIsAtLeast(25, 5)) {
            throw new UsageError(
                `slint-ui pack needs Node.js 25.5 or newer, for --build-sea; this is ${process.version}`,
            );
        }
        await pack(parseOptions(argv));
    } catch (error) {
        if (error instanceof UsageError) {
            console.error(`slint-ui pack: ${error.message}`);
            process.exit(1);
        }
        throw error;
    }
}
