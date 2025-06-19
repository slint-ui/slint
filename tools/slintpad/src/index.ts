// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore cupertino lumino permalink

import { EditorWidget, initialize as initializeEditor } from "./editor_widget";
import { LspWaiter, type Lsp } from "./lsp";
import { PreviewWidget } from "./preview_widget";

import {
    export_to_gist,
    manage_github_access,
    has_github_access_token,
} from "./github";

import {
    report_export_url_dialog,
    report_export_error_dialog,
    export_gist_dialog,
    about_dialog,
} from "./dialogs";

import { CommandRegistry } from "@lumino/commands";
import { Menu, MenuBar, SplitPanel, Widget } from "@lumino/widgets";

const lsp_waiter = new LspWaiter();

const commands = new CommandRegistry();

function create_demo_menu(editor: EditorWidget): Menu {
    const menu = new Menu({ commands });
    menu.title.label = "Open Demo";

    for (const demo of editor.known_demos()) {
        const command_name = "slint:set_demo_" + demo[1];
        commands.addCommand(command_name, {
            label: demo[1],
            execute: () => {
                return editor.set_demo(demo[0]);
            },
        });
        menu.addItem({ command: command_name });
    }
    return menu;
}

function create_settings_menu(): Menu {
    const menu = new Menu({ commands });
    menu.title.label = "Settings";

    commands.addCommand("slint:store_github_token", {
        label: "Manage Github login",
        iconClass: "fa-brands fa-github",
        execute: () => {
            // biome-ignore lint/nursery/noFloatingPromises: <explanation>
            manage_github_access();
        },
    });

    menu.addItem({ command: "slint:store_github_token" });

    return menu;
}

function create_project_menu(
    editor: EditorWidget,
    preview: PreviewWidget,
): Menu {
    const menu = new Menu({ commands });
    menu.title.label = "Project";

    commands.addCommand("slint:open_url", {
        label: "Open URL",
        iconClass: "fa fa-link",
        mnemonic: 1,
        execute: () => {
            const url = prompt("Please enter the URL to open");
            // biome-ignore lint/nursery/noFloatingPromises: <explanation>
            editor.project_from_url(url);
        },
    });

    commands.addKeyBinding({
        keys: ["Accel O"],
        selector: "body",
        command: "slint:open_url",
    });

    commands.addCommand("slint:add_file", {
        label: "Add File",
        iconClass: "fa-regular fa-file",
        mnemonic: 1,
        execute: () => {
            let name = prompt("Please enter the file name");
            if (name == null) {
                return;
            }
            if (!name.endsWith(".slint")) {
                name = name + ".slint";
            }
            editor.add_empty_file_to_project(name);
        },
    });

    commands.addKeyBinding({
        keys: ["Accel N"],
        selector: "body",
        command: "slint:add_file",
    });

    menu.addItem({ command: "slint:open_url" });
    menu.addItem({ type: "submenu", submenu: create_demo_menu(editor) });
    menu.addItem({ type: "separator" });
    menu.addItem({ command: "slint:add_file" });
    menu.addItem({
        type: "submenu",
        submenu: create_share_menu(editor, preview),
    });
    menu.addItem({ type: "separator" });
    menu.addItem({ type: "submenu", submenu: create_settings_menu() });
    menu.addItem({ type: "separator" });

    commands.addCommand("slint:about", {
        label: "About",
        iconClass: "fa-info-circle",
        execute: () => about_dialog(),
    });
    menu.addItem({ command: "slint:about" });

    return menu;
}

function create_share_menu(editor: EditorWidget, preview: PreviewWidget): Menu {
    const menu = new Menu({ commands });
    menu.title.label = "Share";

    commands.addCommand("slint:copy_permalink", {
        label: "Copy Permalink to Clipboard",
        iconClass: "fa fa-share",
        mnemonic: 1,
        isEnabled: () => {
            return editor.open_document_urls.length === 1;
        },
        execute: () => {
            const params = new URLSearchParams();
            params.set("snippet", editor.current_editor_content);
            params.set("style", preview.current_style());
            const this_url = new URL(window.location.toString());
            this_url.search = params.toString();

            report_export_url_dialog(this_url.toString());
        },
    });
    commands.addCommand("slint:create_gist", {
        label: "Export to github Gist",
        iconClass: "fa-brands fa-github",
        mnemonic: 1,
        isEnabled: () => {
            return editor.open_document_urls.length > 0;
        },
        execute: async () => {
            let has_token = has_github_access_token();
            if (!has_token) {
                await manage_github_access();
            }
            has_token = has_github_access_token();

            if (has_token) {
                await export_gist_dialog((desc, is_public) => {
                    export_to_gist(editor, desc, is_public)
                        .then((url) => {
                            const params = new URLSearchParams();
                            params.set("load_url", url);
                            const extra_url = new URL(
                                window.location.toString(),
                            );
                            extra_url.search = params.toString();

                            report_export_url_dialog(url, extra_url.toString());
                        })
                        .catch((e) => report_export_error_dialog(e));
                });
            } else {
                alert(
                    "You need a github access token set up to export as a gist.",
                );
            }
        },
    });

    menu.addItem({ command: "slint:create_gist" });
    menu.addItem({ command: "slint:copy_permalink" });

    return menu;
}

const url_params = new URLSearchParams(window.location.search);
const url_style = url_params.get("style");

function setup(lsp: Lsp) {
    const editor = new EditorWidget(lsp);
    const preview = new PreviewWidget(
        lsp,
        (url: string) => editor.map_url(url),
        url_style ?? "",
    );

    const menu_bar = new MenuBar();
    menu_bar.id = "menuBar";
    menu_bar.addMenu(create_project_menu(editor, preview));

    const main = new SplitPanel({ orientation: "horizontal" });
    main.id = "main";
    main.addWidget(editor);
    main.addWidget(preview);

    window.onresize = () => {
        main.update();
    };

    document.addEventListener("keydown", (event: KeyboardEvent) => {
        commands.processKeydownEvent(event);
    });

    Widget.attach(menu_bar, document.body);
    Widget.attach(main, document.body);
}

function main() {
    initializeEditor()
        .then((_) => {
            lsp_waiter
                .wait_for_lsp()
                .then((lsp) => {
                    setup(lsp);
                    document.body.getElementsByClassName("loader")[0].remove();
                })
                .catch((e) => {
                    console.info("LSP fail:", e);
                    const div = document.createElement("div");
                    div.className = "browser-error";
                    div.innerHTML =
                        "<p>Failed to start the slint language server</p>";
                    document.body.getElementsByClassName("loader")[0].remove();
                    document.body.appendChild(div);
                });
        })
        .catch((e) => {
            console.info("Monaco fail:", e);
        });
}

window.onload = main;
