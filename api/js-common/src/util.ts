// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/**
 * Translates a Slint kebab-case name to a JavaScript snake_case name.
 */
export function translateName(key: string): string {
    return key.replace(/-/g, "_");
}
