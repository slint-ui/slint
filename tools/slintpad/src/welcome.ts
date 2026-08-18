// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Starter projects offered on the first-run screen. The grid is embedded in
// the startup screen (see index.ts); picking a card calls the supplied
// callback with the chosen template.

export interface Template {
    id: string;
    name: string;
    description: string;
    // Exactly one of these is set. `code` is an inline snippet loaded as
    // main.slint; `demo` is a repo-relative path loaded via set_demo (the
    // same demos offered by the toolbar's "Open Demo" menu).
    code?: string;
    demo?: string;
}

const TEMPLATES: Template[] = [
    {
        id: "blank",
        name: "Blank",
        description: "An empty window to build from.",
        code: `// A blank canvas. Start building here.
export component Main inherits Window {
    width: 400px;
    height: 300px;
}
`,
    },
    {
        id: "hello",
        name: "Hello World",
        description: "A greeting and the Slint logo.",
        code: `import { AboutSlint, VerticalBox } from "std-widgets.slint";

export component Main inherits Window {
    VerticalBox {
        Text {
            text: "Hello, Slint!";
            font-size: 28px;
            horizontal-alignment: center;
        }
        AboutSlint {
            preferred-height: 150px;
        }
    }
}
`,
    },
    {
        id: "printer",
        name: "Printer Demo",
        description: "The touchscreen UI for a home printer.",
        demo: "demos/printerdemo/ui/printerdemo.slint",
    },
    {
        id: "energy",
        name: "Energy Monitor",
        description: "A dashboard for a home solar system.",
        demo: "demos/energy-monitor/ui/desktop_window.slint",
    },
    {
        id: "home-automation",
        name: "Home Automation",
        description: "A control panel for a smart home.",
        demo: "demos/home-automation/ui/demo-debug.slint",
    },
    {
        id: "gallery",
        name: "Widget Gallery",
        description: "Every standard widget in one place.",
        demo: "examples/gallery/gallery.slint",
    },
];

/// Build the grid of starter cards. Each card carries `data-template` with the
/// template id so the caller can reflect a pending/queued selection, and calls
/// `on_pick` with the chosen template when clicked.
export function create_welcome_grid(
    on_pick: (template: Template) => void,
): HTMLDivElement {
    const grid = document.createElement("div");
    grid.className = "welcome-grid";

    for (const template of TEMPLATES) {
        const card = document.createElement("button");
        card.type = "button";
        card.className = "welcome-card";
        card.dataset.template = template.id;

        const name = document.createElement("span");
        name.className = "welcome-card-name";
        name.textContent = template.name;

        const desc = document.createElement("span");
        desc.className = "welcome-card-desc";
        desc.textContent = template.description;

        card.appendChild(name);
        card.appendChild(desc);
        card.addEventListener("click", () => on_pick(template));
        grid.appendChild(card);
    }

    return grid;
}
