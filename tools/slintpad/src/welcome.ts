// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// A branded first-run screen offering a few starter projects.
// Picking one calls the supplied callback with the chosen template.

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

const SLINT_MARK = `<svg viewBox="0 0 30 42" fill="#ffffff" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path d="M5.49 41.62 28.6 25.5s1.04-.6 1.04-1.55c0-1.27-1.32-1.68-1.32-1.68L15.61 17.2c-.45-.18-1.08.32-.49.96l4.2 4.24s1.17 1.16 1.17 1.92c0 .76-.72 1.43-.72 1.43L4.46 40.65c-.55.53.19 1.49 1.03.97Z"/><path d="M24.15.76 1.04 16.88S0 17.48 0 18.43c0 1.27 1.32 1.68 1.32 1.68l12.71 5.07c.46.17 1.08-.33.5-.97l-4.21-4.25s-1.17-1.16-1.17-1.92c0-.76.72-1.43.72-1.43L25.17 1.73c.56-.53-.18-1.49-1.02-.97Z"/></svg>`;

/// Show the first-run welcome overlay. `on_pick` receives the chosen
/// template; the overlay removes itself before the callback runs.
export function show_welcome(on_pick: (template: Template) => void): void {
    const overlay = document.createElement("div");
    overlay.className = "welcome-overlay";
    overlay.setAttribute("role", "dialog");
    overlay.setAttribute("aria-label", "Start a new SlintPad project");

    const panel = document.createElement("div");
    panel.className = "welcome-panel";

    const header = document.createElement("div");
    header.className = "welcome-header";
    header.innerHTML = `
        <div class="welcome-mark">${SLINT_MARK}</div>
        <h1 class="welcome-title">Start a new project</h1>
        <p class="welcome-subtitle">Pick a starter and start building. Your changes run live in the preview as you type.</p>
    `;

    const grid = document.createElement("div");
    grid.className = "welcome-grid";

    const dismiss = (template: Template) => {
        overlay.remove();
        on_pick(template);
    };

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
        card.addEventListener("click", () => dismiss(template));
        grid.appendChild(card);
    }

    panel.appendChild(header);
    panel.appendChild(grid);
    overlay.appendChild(build_glow("welcome-glow welcome-glow-a"));
    overlay.appendChild(build_glow("welcome-glow welcome-glow-b"));
    overlay.appendChild(panel);
    document.body.appendChild(overlay);

    // Move focus to the first template for keyboard users.
    (grid.firstElementChild as HTMLElement | null)?.focus();
}

function build_glow(class_name: string): HTMLDivElement {
    const glow = document.createElement("div");
    glow.className = class_name;
    return glow;
}
