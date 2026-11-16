// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

export const PROJECT_FILE_NAME = "slint-project.json";

/** Whether the URL `path` points to a project file (`slint-project.json`). */
export function is_project_url(path: string): boolean {
    return path.split("/").pop() === PROJECT_FILE_NAME;
}
