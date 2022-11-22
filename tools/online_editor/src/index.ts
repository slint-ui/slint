// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

// cSpell: ignore lumino permalink

import { LspRange, LspPosition } from "./lsp_integration";

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

import { EditorWidget } from "./editor_widget";
import { PreviewWidget } from "./preview_widget";
import { OutlineWidget } from "./outline_widget";
import { PropertiesWidget } from "./properties_widget";
import { WelcomeWidget } from "./welcome_widget";

const commands = new CommandRegistry();

const local_storage_key_layout = "layout_v1";

function create_build_menu(): Menu {
    const menu = new Menu({ commands });
    menu.title.label = "Build";

    menu.addItem({ command: "slint:compile" });
    menu.addItem({ command: "slint:auto_compile" });

    return menu;
}

function create_demo_menu(editor: EditorWidget): Menu {
    const menu = new Menu({ commands });
    menu.title.label = "Demos";

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

function create_style_menu(editor: EditorWidget): Menu {
    const menu = new Menu({ commands });
    menu.title.label = "Style";

    for (const style of [
        { label: "Fluent", name: "fluent" },
        { label: "Fluent Light", name: "fluent-light" },
        { label: "Fluent Dark", name: "fluent-dark" },
        { label: "Material", name: "material" },
        { label: "Material Light", name: "material-light" },
        { label: "Material Dark", name: "material-dark" },
    ]) {
        const command_name = "slint:set_style_" + style.name;
        commands.addCommand(command_name, {
            label: style.label,
            isToggled: () => {
                return editor.style === style.name;
            },
            execute: () => {
                editor.style = style.name;
            },
        });
        menu.addItem({ command: command_name });
    }

    return menu;
}

function create_share_menu(editor: EditorWidget): Menu {
    const menu = new Menu({ commands });
    menu.title.label = "Share";

    commands.addCommand("slint:copy_permalink", {
        label: "Copy Permalink to Clipboard",
        iconClass: "fa da-share",
        mnemonic: 1,
        execute: () => {
            const params = new URLSearchParams();
            params.set("snippet", editor.current_editor_content);
            const this_url = new URL(window.location.toString());
            this_url.search = params.toString();
            const share_url = this_url.toString();

            navigator.clipboard.writeText(share_url);
        },
    });

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

function main() {
    commands.addCommand("slint:compile", {
        label: "Compile",
        iconClass: "fa fa-hammer",
        mnemonic: 1,
        execute: () => {
            editor.compile();
        },
    });

    commands.addCommand("slint:auto_compile", {
        label: "Automatically Compile on Change",
        mnemonic: 1,
        isToggled: () => {
            return editor.auto_compile;
        },
        execute: () => {
            editor.auto_compile = !editor.auto_compile;
        },
    });

    commands.addKeyBinding({
        keys: ["Accel B"],
        selector: "body",
        command: "slint:compile",
    });

    const editor = new EditorWidget();
    const dock = new DockPanel();

    const dock_widgets = new DockWidgets(
        dock,
        [
            () => {
                const preview = new PreviewWidget();
                editor.onRenderRequest = (style, source, url, fetcher) => {
                    return preview.render(style, source, url, fetcher);
                };

                commands.execute("slint:compile");
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
                const outline = new OutlineWidget(editor.position, () => {
                    return [
                        editor.language_client,
                        editor.current_text_document_uri,
                    ];
                });

                editor.onPositionChange = (position) => {
                    outline.position_changed(position);
                };

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
                editor.onNewPropertyData = (p) => {
                    properties.set_properties(p);
                };

                properties.on_goto_position = (uri, pos) => {
                    editor.goto_position(uri, pos);
                };
                properties.replace_text_function = (
                    uri,
                    range,
                    new_text,
                    validator,
                ) => {
                    return editor.replace_text(uri, range, new_text, validator);
                };

                return properties;
            },
            { mode: "tab-after", ref: "Welcome" },
        ],
    );

    const menu_bar = new MenuBar();
    menu_bar.id = "menuBar";
    menu_bar.addMenu(create_share_menu(editor));
    menu_bar.addMenu(create_build_menu());
    menu_bar.addMenu(create_demo_menu(editor));
    menu_bar.addMenu(create_style_menu(editor));
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

    editor.editor_ready.then(() => {
        document.body.getElementsByClassName("loader")[0].remove();
    });

    Widget.attach(menu_bar, document.body);
    Widget.attach(main, document.body);
}

window.onload = main;
