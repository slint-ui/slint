// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore cupertino lumino permalink

import { EditorWidget, initialize as initializeEditor } from "./editor_widget";
import { LspWaiter, type Lsp, type LoadPhase, type PanelLayout } from "./lsp";
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
    set_panic_share_url_getter,
} from "./dialogs";

import { CommandRegistry } from "@lumino/commands";
import { Menu, MenuBar, SplitPanel, Widget } from "@lumino/widgets";

import { type InvokeSlintpadCallback, SlintPadCallbackFunction } from "./lsp";

const loader = document.getElementById("loader");
const loader_message = document.getElementById("loader-message");
const loader_progress = document.getElementById(
    "loader-progress",
) as HTMLProgressElement | null;

function update_loader(phase: LoadPhase) {
    if (loader_message === null || loader_progress === null) {
        return;
    }
    if (phase.kind === "downloading") {
        if (phase.total) {
            const percent = Math.round((phase.received / phase.total) * 100);
            loader_message.textContent = `Downloading Slint runtime… ${percent}%`;
            loader_progress.max = phase.total;
            loader_progress.value = phase.received;
        } else {
            const kb = Math.round(phase.received / 1024);
            loader_message.textContent = `Downloading Slint runtime… ${kb} KB`;
            loader_progress.removeAttribute("value");
        }
    } else if (phase.kind === "compiling") {
        loader_message.textContent = "Compiling…";
        loader_progress.removeAttribute("value");
    } else if (phase.kind === "initializing") {
        loader_message.textContent = "Initializing…";
        loader_progress.removeAttribute("value");
    }
}

const lsp_waiter = new LspWaiter(update_loader);

const commands = new CommandRegistry();

const url_params = new URLSearchParams(window.location.search);
const url_style = url_params.get("style");

const PANEL_LAYOUT_KEY = "slintpad-panel-layout";

function load_panel_layout(): PanelLayout | null {
    try {
        const raw = window.localStorage.getItem(PANEL_LAYOUT_KEY);
        if (raw === null) {
            return null;
        }
        const parsed = JSON.parse(raw);
        return {
            library: !!parsed.library,
            properties: !!parsed.properties,
            outline: !!parsed.outline,
            data: !!parsed.data,
            console: !!parsed.console,
        };
    } catch (_) {
        return null;
    }
}

function save_panel_layout(layout: PanelLayout): void {
    try {
        window.localStorage.setItem(PANEL_LAYOUT_KEY, JSON.stringify(layout));
    } catch (_) {
        // Ignore storage failures (e.g. private mode with a full quota).
    }
}

function setup(lsp: Lsp) {
    const editor = new EditorWidget(lsp);
    set_panic_share_url_getter(() => editor.share_url());
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
            } else if (func_type === SlintPadCallbackFunction.NewFile) {
                void editor.set_demo("");
            } else if (func_type === SlintPadCallbackFunction.SavePanelLayout) {
                save_panel_layout(args as PanelLayout);
            }
        },
        (previewer) => {
            const layout = load_panel_layout();
            if (layout !== null) {
                previewer.restore_panels(layout);
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
            if (loader_message !== null) {
                loader_message.textContent = "Starting language server…";
            }
            lsp_waiter
                .wait_for_lsp()
                .then((lsp) => {
                    setup(lsp);
                    loader?.remove();
                })
                .catch((e) => {
                    console.info("LSP fail:", e);
                    const div = document.createElement("div");
                    div.className = "browser-error";
                    div.innerHTML =
                        "<p>Failed to start the slint language server</p>";
                    loader?.remove();
                    document.body.appendChild(div);
                });
        })
        .catch((e) => {
            console.info("Monaco fail:", e);
        });
}

window.onload = main;
