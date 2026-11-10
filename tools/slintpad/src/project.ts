// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

export const PROJECT_FILE_NAME = "slint-project.json";

/** Whether `url` points to a project file (`slint-project.json`). */
export function is_project_url(url: URL): boolean {
    return url.pathname.split("/").pop() === PROJECT_FILE_NAME;
}

/** A project file and its `main` file; the compiler applies its other settings. */
export interface Project {
    source: string;
    main: URL;
}

/** Downloads the project file at `url`. */
export async function load_project(url: URL): Promise<Project> {
    const response = await fetch(url);
    if (!response.ok) {
        throw new Error(
            `Failed to download ${url}: ${response.status} ${response.statusText}`,
        );
    }
    const source = await response.text();
    const data = JSON.parse(blank_out_comments(source));
    if (typeof data?.main !== "string") {
        throw new Error(`${url} has no 'main' file`);
    }
    return { source, main: new URL(data.main, url) };
}

// Project files may contain `//` and `/* */` comments.
function blank_out_comments(source: string): string {
    return source.replace(
        /("(?:[^"\\]|\\.)*")|\/\/[^\n]*|\/\*[\s\S]*?(?:\*\/|$)/g,
        (match, string_literal) =>
            string_literal ?? match.replace(/[^\n]/g, " "),
    );
}
