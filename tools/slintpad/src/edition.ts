// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { version as slint_version } from "../package.json";

const release_edition = slint_version.split(".").slice(0, 2).join(".");

// Same syntax as `SlintEdition` in the compiler: no leading zeros.
function parse(edition: string): [number, number] | undefined {
    const match = /^(0|[1-9]\d*)\.(0|[1-9]\d*)$/.exec(edition);
    return match ? [Number(match[1]), Number(match[2])] : undefined;
}

/**
 * The `?edition=` URL parameter, or the edition of this SlintPad release
 * when the parameter is missing, malformed, or newer than this release.
 */
export function edition_from_url(params: URLSearchParams): string {
    const edition = params.get("edition");
    const requested = edition ? parse(edition) : undefined;
    const [major, minor] = parse(release_edition) ?? [0, 0];
    if (
        edition &&
        requested &&
        (requested[0] < major ||
            (requested[0] === major && requested[1] <= minor))
    ) {
        return edition;
    }
    return release_edition;
}
