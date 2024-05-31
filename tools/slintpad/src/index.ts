// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore cupertino lumino permalink

import { EditorWidget, initialize as initializeEditor } from "./editor_widget";
import { LspWaiter, Lsp } from "./lsp";
import { LspRange, LspPosition } from "./lsp_integration";
import { OutlineWidget } from "./outline_widget";
import { PreviewWidget } from "./preview_widget";
import { PropertiesWidget } from "./properties_widget";
import { WelcomeWidget } from "./welcome_widget";

import {
    export_to_gist,
    manage_github_access,
    has_github_access_token,
} from "./github";

import {
    report_export_url_dialog,
    report_export_error_dialog,
    export_gist_dialog,
} from "./dialogs";

import { CommandRegistry } from "@lumino/commands";
import {
    DockLayout,
    DockPanel,
    Layout,
    Menu,
    MenuBar,
    SplitPanel,
    Widget,
} from "@lumino/widgets";

const lsp_waiter = new LspWaiter();

const commands = new CommandRegistry();

const local_storage_key_layout = "layout_v1";

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
            return editor.open_document_urls.length == 1;
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

function widget_pseudo_url(id: string): string {
    return "::widget:://" + id;
}

function id_from_pseudo_url(url: string): string {
    const id = url.substring(14);
    console.assert(url == widget_pseudo_url(id));
    return id;
}

function create_view_menu(dock_widgets: DockWidgets): Menu {
    const dock = dock_widgets.dock;

    const menu = new Menu({ commands });
    menu.title.label = "Views";

    commands.addCommand("slint:save-dock-layout", {
        label: "Save Dock Layout",
        caption: "Save the current dock layout",
        execute: () => {
            const layout = dock.saveLayout();
            const widgets = Array.from(dock.widgets());

            const objMap: Map<object, string> = new Map();
            for (const w of widgets) {
                objMap.set(w, widget_pseudo_url(w.title.label));
            }
            objMap.set(dock, "::widget:://DockPanel");
            if (dock.layout != null) {
                objMap.set(dock.layout, "::object:://DockLayout");
            }
            if (dock.parent != null) {
                objMap.set(dock.parent, "::widget:://parent");
            }
            try {
                const layout_str = JSON.stringify(layout, (_, value) => {
                    const result = objMap.get(value);
                    if (result === undefined) {
                        return value;
                    }
                    return result;
                });
                localStorage.setItem(local_storage_key_layout, layout_str);
            } catch (e) {
                console.log("Failed to save dock layout!", e);
            }
        },
    });

    commands.addCommand("slint:reset-dock-layout", {
        label: "Reset to Default Layout",
        caption: "Reset to default layout",
        isEnabled: () => {
            return localStorage.getItem(local_storage_key_layout) != null;
        },
        execute: () => {
            localStorage.removeItem(local_storage_key_layout);
        },
    });

    commands.addCommand("slint:restore-dock-layout", {
        label: "Restore Dock Layout",
        caption: "Restore the stored Dock layout",
        isEnabled: () => {
            return localStorage.getItem(local_storage_key_layout) != null;
        },
        execute: () => {
            const layout_str = localStorage.getItem(local_storage_key_layout);
            if (layout_str != null && layout_str != "") {
                const idMap: Map<string, Widget | Layout | null> = new Map();
                for (const id of dock_widgets.ids) {
                    idMap.set(widget_pseudo_url(id), dock_widgets.widget(id));
                }
                idMap.set("::widget:://DockPanel", dock);
                idMap.set("::object:://DockLayout", dock.layout);
                idMap.set("::object:://parent", dock.parent);

                try {
                    const layout = JSON.parse(layout_str, (_, value) => {
                        const obj = idMap.get(value);
                        if (obj === undefined) {
                            // nothing we need to map!
                            return value;
                        }
                        if (obj === null) {
                            // We need to create this first!
                            return dock_widgets.create(
                                id_from_pseudo_url(value),
                            );
                        } else {
                            return obj;
                        }
                    });
                    dock.restoreLayout(layout);
                } catch (e) {
                    console.log("Failed to restore layout!", e);
                }
            }
        },
    });

    menu.addItem({ command: "slint:save-dock-layout" });
    menu.addItem({ command: "slint:restore-dock-layout" });
    menu.addItem({ command: "slint:reset-dock-layout" });
    menu.addItem({ type: "separator" });

    for (const w of dock.widgets()) {
        const id = w.title.label;
        const command_name = "slint:visibility_" + id;
        commands.addCommand(command_name, {
            label: "Show " + w.title.label,
            isToggled: () => {
                return dock_widgets.widget(id) != null;
            },
            execute: () => {
                const widget = dock_widgets.widget(id);
                if (widget == null) {
                    dock_widgets.create(id);
                } else {
                    widget.close();
                }
            },
        });

        menu.addItem({ command: command_name });
    }

    return menu;
}

class DockWidgets {
    #factories: Map<string, () => Widget> = new Map();
    #dock: DockPanel;

    constructor(
        dock: DockPanel,
        ...factories: [() => Widget, any][] // eslint-disable-line
    ) {
        this.#dock = dock;

        for (const data of factories) {
            const factory = data[0];
            const widget = factory();
            const id = widget.title.label;
            const layout = data[1];

            console.assert(!this.#factories.has(id));
            this.#factories.set(id, factory);
            const ref = layout.ref;
            if (typeof ref === "string") {
                layout.ref = this.widget(ref);
            }
            console.assert(widget.title.label === id);
            dock.addWidget(widget, layout as DockLayout.IAddOptions);
        }
    }

    create(id: string): Widget | null {
        const factory = this.#factories.get(id);
        if (factory != null) {
            const widget = factory();
            this.#dock.addWidget(widget);
            return widget;
        } else {
            return null;
        }
    }

    widget(id: string): Widget | null {
        for (const w of this.#dock.widgets()) {
            if (w.title.label == id) {
                return w;
            }
        }
        return null;
    }

    get dock(): DockPanel {
        return this.#dock;
    }

    get ids(): string[] {
        return Array.from(this.#factories.keys());
    }
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

    const dock = new DockPanel();

    const dock_widgets = new DockWidgets(
        dock,
        [
            () => {
                return preview;
            },
            {},
        ],
        [
            () => {
                return new WelcomeWidget();
            },
            { mode: "split-bottom", ref: "Preview" },
        ],
        [
            () => {
                const outline = new OutlineWidget(
                    editor.position,
                    () => editor.language_client,
                );

                outline.on_goto_position = (
                    uri: string,
                    pos: LspPosition | LspRange,
                ) => {
                    editor.goto_position(uri, pos);
                };

                return outline;
            },
            { mode: "tab-after", ref: "Welcome" },
        ],
        [
            () => {
                const properties = new PropertiesWidget();

                const pos = editor.position;
                properties.position_changed(pos.uri, pos.version, pos.position);

                properties.on_goto_position = (uri, pos) => {
                    editor.goto_position(uri, pos);
                };

                return properties;
            },
            { mode: "tab-after", ref: "Welcome" },
        ],
    );

    editor.onPositionChange = (pos) => {
        (dock_widgets.widget("Outline") as OutlineWidget)?.position_changed(
            pos,
        );
        (
            dock_widgets.widget("Properties") as PropertiesWidget
        )?.position_changed(pos.uri, pos.version, pos.position);
    };

    const menu_bar = new MenuBar();
    menu_bar.id = "menuBar";
    menu_bar.addMenu(create_project_menu(editor, preview));
    menu_bar.addMenu(create_view_menu(dock_widgets));

    const main = new SplitPanel({ orientation: "horizontal" });
    main.id = "main";
    main.addWidget(editor);
    main.addWidget(dock);

    commands.execute("slint:restore-dock-layout");

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
