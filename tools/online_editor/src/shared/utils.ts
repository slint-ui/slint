// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

export function extract_uri_from_progress_message(input: string): string {
    const start = input.indexOf(": ");
    const end = input.lastIndexOf("@");
    return input.slice(start + 2, end);
}
