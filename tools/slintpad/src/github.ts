// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { EditorWidget, UrlMapper, KnownUrlMapper } from "./editor_widget";
import { modal_dialog } from "./dialogs";

const local_storage_key_github_token = "github_token_v1";

export function has_github_access_token(): boolean {
    const token = localStorage.getItem(local_storage_key_github_token);
    return token != null && token !== "";
}

export async function manage_github_access(): Promise<boolean | null> {
    return new Promise((resolve, _) => {
        let result: boolean | null = null;

        let new_access_token = "";

        const is_valid_token = (t: string) =>
            t.match(/^github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}$/);

        modal_dialog(
            "manage_github_dialog",
            (ready_callback) => {
                ready_callback(true);

                const description_div = document.createElement("div");
                description_div.classList.add("description_area");

                const description = document.createElement("p");

                description_div.appendChild(description);

                const state_div = document.createElement("div");
                state_div.classList.add("current_state");

                const token_input = document.createElement("input");
                token_input.classList.add("token_input");
                token_input.type = "text";
                token_input.pattern =
                    "^github_pat_[a-zA-Z0-9]{22}_[a-zA-Z0-9]{59}$";
                token_input.oninput = () => {
                    const valid = token_input.reportValidity();
                    ready_callback(valid);
                    if (valid) {
                        new_access_token = token_input.value;
                    } else {
                        new_access_token = "";
                    }
                };

                const forget_button = document.createElement("button");
                forget_button.classList.add("forget");
                forget_button.classList.add("button");

                forget_button.innerText = "forget token";

                function set_state(nt: string) {
                    new_access_token = nt;

                    if (new_access_token != "") {
                        token_input.style.display = "none";
                        token_input.value = "";
                        token_input.readOnly = true;
                        forget_button.style.display = "block";

                        description.innerText =
                            "You have a github access token set up.";
                    } else {
                        description.innerHTML =
                            "You have no github access token set up.<br>Visit your github account, " +
                            "go to your settings, then developer settings and create a personal access " +
                            "token there with the permission to read and write Gists. Then paste it into " +
                            "the text field below.";
                        token_input.placeholder =
                            "Github personal access token";
                        token_input.value = "";
                        token_input.style.display = "block";
                        token_input.readOnly = false;
                        forget_button.style.display = "none";
                    }

                    token_input.value = "";
                }

                set_state(get_github_access_token() ?? "");

                forget_button.onclick = () => {
                    set_state("");
                };

                state_div.appendChild(token_input);
                state_div.appendChild(forget_button);

                return [description_div, state_div];
            },
            "OK",
            () => {
                if (
                    is_valid_token(new_access_token) ||
                    new_access_token == ""
                ) {
                    localStorage.setItem(
                        local_storage_key_github_token,
                        new_access_token,
                    );
                }
                result = has_github_access_token();
            },
            () => {
                resolve(result);
            },
        );
    });
}

function get_github_access_token(): string | null {
    return localStorage.getItem(local_storage_key_github_token);
}

export async function export_to_gist(
    editor: EditorWidget,
    description: string,
    is_public: boolean,
): Promise<string> {
    const access_token = get_github_access_token();
    console.assert(access_token != null);

    // collect data:
    const files: { [key: string]: { [key: string]: string } } = {};
    const urls = editor.open_document_urls;
    if (urls.length == 0) {
        return Promise.reject("Nothing to export");
    }

    const to_strip = urls[0].lastIndexOf("/") + 1;

    for (const u of urls) {
        files[u.slice(to_strip)] = {
            content: editor.document_contents(u) ?? "",
        };
    }

    const extras: { [path: string]: string } = {};
    Object.entries(editor.extra_files).forEach(async ([f, u]) => {
        extras[f.slice(1)] = u;
    });

    const project_data = {
        main: urls[0].slice(to_strip),
        mappings: extras,
    };

    files["slint.json"] = { content: JSON.stringify(project_data) };

    const data = JSON.stringify({
        description: description,
        public: is_public,
        files: files,
    });

    const response = await fetch("https://api.github.com/gists", {
        method: "POST",
        mode: "cors",
        cache: "no-cache",
        credentials: "same-origin",
        headers: {
            "Content-Type": "application/json",
            Accept: "application/vnd.github+json",
            Authorization: "Bearer " + access_token,
        },
        redirect: "follow",
        referrerPolicy: "no-referrer",
        body: data,
    });

    if (response.ok) {
        const body = await response.json();
        if (body.errors) {
            for (const e of body.errors) {
                console.error(JSON.stringify(e));
            }
            return Promise.reject(
                "Failed to publish to Github:\n" + body.message,
            );
        } else if (body.html_url == null) {
            return Promise.reject(
                "Failed to retrieve URL after publishing to Github",
            );
        } else {
            return Promise.resolve(body.html_url);
        }
    } else {
        return Promise.reject(
            "Failed to publish a Gist to Github with status code:" +
                response.status +
                "\n" +
                response.statusText,
        );
    }
}

async function _process_gist_url(
    uuid: string,
    url: URL,
): Promise<[string, string | null, UrlMapper | null]> {
    const path = url.pathname.split("/");

    // A URL to a Gist, not to a specific file in a gist!
    if (path.length == 3 || path.length == 2) {
        // Raw gist URL: Find a start file!
        const gist_id = path[path.length - 1];

        try {
            // eslint-disable-next-line @typescript-eslint/no-explicit-any
            const headers: any = {
                Accept: "application/vnd.github+json",
            };
            const token = get_github_access_token();
            if (token != null) {
                headers.Authorization = "Bearer " + token;
            }
            const response = await fetch(
                "https://api.github.com/gists/" + gist_id,
                {
                    method: "GET",
                    headers: headers,
                },
            );
            const body = await response.json();

            const map: { [path: string]: string } = {};
            let definite_main_file_name;
            let fallback_main_file_name;
            let fallback_main_file_url;
            for (const [k, v] of Object.entries(body.files)) {
                if (k === "slint.json") {
                    // eslint-disable-next-line @typescript-eslint/no-explicit-any
                    const content: any = JSON.parse(
                        // eslint-disable-next-line @typescript-eslint/no-explicit-any
                        (v as any).content as string,
                    );
                    definite_main_file_name = content.main as string;
                    const mappings = content.mappings as {
                        string: string;
                    };

                    Object.entries(mappings).forEach(([f, u]) => {
                        map["/" + f] = u;
                    });
                } else {
                    // eslint-disable-next-line @typescript-eslint/no-explicit-any
                    const url = (v as any).raw_url;
                    if (fallback_main_file_name == null) {
                        fallback_main_file_name = k;
                        fallback_main_file_url = url;
                    }
                    map["/" + k] = url;
                }
            }

            const mapper = new KnownUrlMapper(uuid, map);

            if (body.errors) {
                return Promise.reject(
                    "Failed to read gist:\n" + body.errors.join("\n"),
                );
            }

            const description_file =
                body.description.match(/main file is: "(.+)"/i)?.[1];
            let main_file_name =
                definite_main_file_name ?? description_file ?? "main.slint";

            let main_file_url = map["/" + main_file_name];
            if (main_file_url == null) {
                main_file_name = fallback_main_file_name;
                main_file_url = fallback_main_file_url;
            }

            return Promise.resolve([
                main_file_url,
                "/" + main_file_name,
                mapper,
            ]);
        } catch (e) {
            return Promise.reject(
                "Failed to retrieve information on Gist:\n" + e,
            );
        }
    }

    return Promise.resolve([url.toString(), null, null]);
}

async function _process_github_url(
    _uuid: string,
    url: URL,
): Promise<[string, null, null]> {
    const path = url.pathname.split("/");

    if (path[3] === "blob") {
        path.splice(3, 1);

        return Promise.resolve([
            url.protocol + "//raw.githubusercontent.com" + path.join("/"),
            null,
            null,
        ]);
    } else {
        return Promise.resolve([url.toString(), null, null]);
    }
}

export async function open_url(
    uuid: string,
    url_string: string,
): Promise<[string | null, string | null, UrlMapper | null]> {
    try {
        const url = new URL(url_string);

        if (url.hostname === "gist.github.com") {
            return _process_gist_url(uuid, url);
        }
        if (url.hostname === "github.com") {
            return _process_github_url(uuid, url);
        }
    } catch (_) {
        return Promise.reject("Failed to process URL");
    }
    return Promise.resolve([null, null, null]);
}
