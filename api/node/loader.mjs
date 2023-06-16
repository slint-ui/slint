// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.0 OR LicenseRef-Slint-commercial

import { URL, pathToFileURL } from 'url';

const extensionsRegex = /\.(60|slint)$/;
const baseURL = pathToFileURL(`${process.cwd()}/`).href;

export function resolve(specifier, context, defaultResolve) {

    const { parentURL = baseURL } = context;

    if (extensionsRegex.test(specifier)) {
        return { url: new URL(specifier, parentURL).href };
    }

    return defaultResolve(specifier, context, defaultResolve);
}


export function getFormat(url, context, defaultGetFormat) {
    if (extensionsRegex.test(url)) {
        return {
            format: 'module'
        };
    }
    return defaultGetFormat(url, context, defaultGetFormat);
}

export function transformSource(source, context, defaultTransformSource) {
    const { url, format } = context;

    if (extensionsRegex.test(url)) {
        console.log(`This is where one can compile ${url}`)
        return {
            source: "console.log('Hey'); export function foo(x) { return x + 55 }"
        };
    }

    // Let Node.js handle all other sources.
    return defaultTransformSource(source, context, defaultTransformSource);
}
