// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore cupertino lumino permalink

import { EditorWidget, initialize as initializeEditor } from "./editor_widget";
import { LspWaiter, type Lsp, type LoadPhase } from "./lsp";
import { PreviewWidget } from "./preview_widget";
import { create_welcome_grid, type Template } from "./welcome";

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
    let text: string;
    if (phase.kind === "downloading") {
        if (phase.total) {
            const percent = Math.round((phase.received / phase.total) * 100);
            text = `Downloading Slint runtime… ${percent}%`;
            loader_progress.max = phase.total;
            loader_progress.value = phase.received;
        } else {
            const kb = Math.round(phase.received / 1024);
            text = `Downloading Slint runtime… ${kb} KB`;
            loader_progress.removeAttribute("value");
        }
    } else if (phase.kind === "compiling") {
        text = "Compiling…";
        loader_progress.removeAttribute("value");
    } else {
        text = "Initializing…";
        loader_progress.removeAttribute("value");
    }
    // Once a starter is chosen, keep the runtime status tied to that choice so
    // the queued pick is clearly what we're waiting on.
    loader_message.textContent =
        pending_template !== null ? `Starting ${pending_template.name}…` : text;
}

const lsp_waiter = new LspWaiter(update_loader);

const commands = new CommandRegistry();

const url_params = new URLSearchParams(window.location.search);
const url_style = url_params.get("style");

// --- Unified startup screen ---
// The loader doubles as the first-run welcome. On a fresh visit (no content in
// the URL and nothing to restore) the starter templates render on the loader
// while the runtime downloads. Picking a card before the runtime is ready
// queues the choice and applies it the moment it is.

const has_content_param = ["snippet", "gz", "load_url", "load_demo"].some(
    (key) => url_params.get(key),
);
const history_files = (window.history.state?.files ?? {}) as {
    [path: string]: string;
};
const is_first_run =
    !has_content_param && Object.keys(history_files).length === 0;

let pending_template: Template | null = null;
let apply_template: ((template: Template) => void) | null = null;

function pick_template(template: Template) {
    if (apply_template !== null) {
        apply_template(template);
        loader?.remove();
        return;
    }
    // Runtime not ready yet: remember the choice, mark the card as queued, and
    // dim the rest. The pick is applied the moment the runtime is ready.
    pending_template = template;
    loader?.classList.add("loader--committed");
    loader?.querySelectorAll(".welcome-card").forEach((card) => {
        card.classList.toggle(
            "welcome-card--queued",
            (card as HTMLElement).dataset.template === template.id,
        );
    });
    if (loader_message !== null) {
        loader_message.textContent = `Starting ${template.name}…`;
    }
}

function enter_welcome_mode() {
    // The loader already lays out logo -> loading strip -> tagline. On first run
    // we insert the welcome header and starter grid between the logo and the
    // strip, so the loading-then-tagline order stays uniform with the plain
    // loader.
    const strip = loader?.querySelector(".loader-strip");
    if (!loader || !strip) {
        return;
    }
    loader.classList.add("loader--welcome");

    const title = document.createElement("h1");
    title.className = "loader-title";
    title.textContent = "Start a new project";

    const subtitle = document.createElement("p");
    subtitle.className = "loader-subtitle";
    subtitle.textContent = "Pick a starter and start building.";

    const grid = create_welcome_grid(pick_template);

    strip.insertAdjacentElement("beforebegin", title);
    strip.insertAdjacentElement("beforebegin", subtitle);
    strip.insertAdjacentElement("beforebegin", grid);
}

function mark_runtime_ready() {
    // Runtime ready with no starter chosen yet. Complete the progress bar and
    // let the strip recede in place (see CSS) so the layout does not jump.
    loader?.classList.add("loader--ready");
    if (loader_progress !== null) {
        loader_progress.max = 1;
        loader_progress.value = 1;
    }
    if (loader_message !== null) {
        loader_message.textContent = "Ready";
    }
}

function setup(lsp: Lsp) {
    const editor = new EditorWidget(lsp);
    set_panic_share_url_getter(() => editor.share_url());
    apply_template = (template) => editor.apply_template(template);
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

// On a fresh visit, add the welcome header and starters to the loader as early
// as possible (this module runs right after the DOM is parsed, before
// window.onload). Returning visitors keep the plain loader as authored in HTML.
if (is_first_run) {
    enter_welcome_mode();
}

function main() {
    initializeEditor()
        .then((_) => {
            if (loader_message !== null && !is_first_run) {
                loader_message.textContent = "Starting language server…";
            }
            lsp_waiter
                .wait_for_lsp()
                .then((lsp) => {
                    setup(lsp);
                    if (!is_first_run) {
                        loader?.remove();
                    } else if (pending_template !== null) {
                        // Chosen while the runtime was still loading.
                        apply_template?.(pending_template);
                        loader?.remove();
                    } else {
                        mark_runtime_ready();
                    }
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
