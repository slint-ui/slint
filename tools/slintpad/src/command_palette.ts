// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore cmdk Archivo

// A ⌘K / Ctrl+K command palette: fuzzy-ish search over demos and actions.

export interface Command {
    id: string;
    title: string;
    hint?: string;
    run: () => void;
}

export interface CommandPalette {
    open: () => void;
}

// Self-contained styles (literal brand colors) so the palette does not
// depend on any external stylesheet.
const CMDK_STYLES = `
.cmdk-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    display: flex;
    align-items: flex-start;
    justify-content: center;
    padding-top: 12vh;
    background: rgba(6, 10, 14, 0.6);
    backdrop-filter: blur(4px);
    font-family: "Archivo", system-ui, -apple-system, "Segoe UI", sans-serif;
}
.cmdk-panel {
    width: min(560px, 92vw);
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: #0f1519;
    border: 1px solid rgba(255, 255, 255, 0.18);
    border-radius: 14px;
    box-shadow: 0 30px 80px rgba(5, 10, 14, 0.6);
    overflow: hidden;
}
.cmdk-input {
    width: 100%;
    box-sizing: border-box;
    border: none;
    border-bottom: 1px solid rgba(255, 255, 255, 0.12);
    background: transparent;
    color: #eef3f5;
    font: inherit;
    font-size: 15px;
    padding: 15px 18px;
    outline: none;
}
.cmdk-input::placeholder {
    color: #9fb0b8;
}
.cmdk-list {
    overflow-y: auto;
    padding: 6px;
}
.cmdk-empty {
    padding: 16px 18px;
    color: #9fb0b8;
    font-size: 14px;
}
.cmdk-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    width: 100%;
    text-align: left;
    border: none;
    background: transparent;
    color: #9fb0b8;
    font: inherit;
    font-size: 14px;
    padding: 10px 12px;
    border-radius: 12px;
    cursor: pointer;
}
.cmdk-row.cmdk-active {
    background: rgba(35, 121, 244, 0.16);
    color: #eef3f5;
}
.cmdk-title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}
.cmdk-hint {
    flex-shrink: 0;
    font-family: "JetBrains Mono", ui-monospace, "Source Code Pro", monospace;
    font-size: 11px;
    color: #9fb0b8;
    border: 1px solid rgba(255, 255, 255, 0.18);
    border-radius: 999px;
    padding: 2px 9px;
}
`;

function inject_styles(): void {
    if (document.getElementById("cmdk-styles") !== null) {
        return;
    }
    const style = document.createElement("style");
    style.id = "cmdk-styles";
    style.textContent = CMDK_STYLES;
    document.head.appendChild(style);
}

export function install_command_palette(commands: Command[]): CommandPalette {
    inject_styles();
    let overlay: HTMLDivElement | null = null;
    let input: HTMLInputElement | null = null;
    let list: HTMLDivElement | null = null;
    let filtered: Command[] = [];
    let active = 0;

    const close = () => {
        overlay?.remove();
        overlay = null;
    };

    const run_active = () => {
        const cmd = filtered[active];
        close();
        cmd?.run();
    };

    const render = () => {
        if (list === null || input === null) {
            return;
        }
        const query = input.value.trim().toLowerCase();
        filtered = query
            ? commands.filter((c) => c.title.toLowerCase().includes(query))
            : commands;
        if (active >= filtered.length) {
            active = Math.max(0, filtered.length - 1);
        }
        list.innerHTML = "";
        if (filtered.length === 0) {
            const empty = document.createElement("div");
            empty.className = "cmdk-empty";
            empty.textContent = "No matching commands";
            list.appendChild(empty);
            return;
        }
        filtered.forEach((cmd, i) => {
            const row = document.createElement("button");
            row.type = "button";
            row.className = i === active ? "cmdk-row cmdk-active" : "cmdk-row";
            const title = document.createElement("span");
            title.className = "cmdk-title";
            title.textContent = cmd.title;
            row.appendChild(title);
            if (cmd.hint) {
                const hint = document.createElement("span");
                hint.className = "cmdk-hint";
                hint.textContent = cmd.hint;
                row.appendChild(hint);
            }
            row.addEventListener("mousemove", () => {
                if (active !== i) {
                    active = i;
                    render();
                }
            });
            row.addEventListener("click", () => {
                active = i;
                run_active();
            });
            list?.appendChild(row);
        });
    };

    const scroll_active_into_view = () => {
        list?.children[active]?.scrollIntoView({ block: "nearest" });
    };

    const open = () => {
        if (overlay !== null) {
            return;
        }
        active = 0;

        overlay = document.createElement("div");
        overlay.className = "cmdk-overlay";
        overlay.setAttribute("role", "dialog");
        overlay.setAttribute("aria-label", "Command palette");

        const panel = document.createElement("div");
        panel.className = "cmdk-panel";

        input = document.createElement("input");
        input.className = "cmdk-input";
        input.type = "text";
        input.placeholder = "Type a command or demo…";
        input.setAttribute("aria-label", "Command palette search");
        input.autocomplete = "off";
        input.spellcheck = false;

        list = document.createElement("div");
        list.className = "cmdk-list";

        panel.appendChild(input);
        panel.appendChild(list);
        overlay.appendChild(panel);
        document.body.appendChild(overlay);

        overlay.addEventListener("mousedown", (e) => {
            if (e.target === overlay) {
                close();
            }
        });

        input.addEventListener("input", () => {
            active = 0;
            render();
        });

        input.addEventListener("keydown", (e) => {
            if (e.key === "ArrowDown") {
                e.preventDefault();
                active = Math.min(active + 1, filtered.length - 1);
                render();
                scroll_active_into_view();
            } else if (e.key === "ArrowUp") {
                e.preventDefault();
                active = Math.max(active - 1, 0);
                render();
                scroll_active_into_view();
            } else if (e.key === "Enter") {
                e.preventDefault();
                run_active();
            } else if (e.key === "Escape") {
                e.preventDefault();
                close();
            }
        });

        render();
        input.focus();
    };

    // Global shortcut. Capture phase so it wins over the editor's own ⌘K
    // chord handling.
    document.addEventListener(
        "keydown",
        (e: KeyboardEvent) => {
            if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
                e.preventDefault();
                e.stopPropagation();
                if (overlay === null) {
                    open();
                } else {
                    close();
                }
            }
        },
        { capture: true },
    );

    return { open };
}
