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

import { type InvokeSlintpadCallback, SlintPadCallbackFunction } from "./lsp";

const lsp_waiter = new LspWaiter();

const commands = new CommandRegistry();

const url_params = new URLSearchParams(window.location.search);
const url_style = url_params.get("style");

function setup(lsp: Lsp) {
    const editor = new EditorWidget(lsp);
    const preview = new PreviewWidget(
        lsp,
        (url: string) => editor.map_url(url),
        url_style ?? "",
        (func_type, args) => {
            if (func_type === SlintPadCallbackFunction.OpenDemoUrl) {
                void editor.set_demo(args as string);
            } else if (func_type === SlintPadCallbackFunction.ShowAbout) {
                about_dialog();
            } else if (func_type === SlintPadCallbackFunction.CopyPermalink) {
                void editor.copy_permalink_to_clipboard();
            }
        },
    );

    const main = new SplitPanel({ orientation: "horizontal" });
    main.id = "main";
    main.addWidget(preview);
    main.addWidget(editor);

    window.onresize = () => {
        main.update();
    };

    document.addEventListener("keydown", (event: KeyboardEvent) => {
        commands.processKeydownEvent(event);
    });

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
