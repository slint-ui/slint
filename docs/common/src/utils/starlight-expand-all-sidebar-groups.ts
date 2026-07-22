// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

type SidebarEntry = string | SidebarGroup;

interface SidebarGroup {
    collapsed?: boolean;
    autogenerate?: {
        collapsed?: boolean;
        directory?: string;
        [key: string]: unknown;
    };
    items?: SidebarEntry[];
    [key: string]: unknown;
}

/**
 * Starlight plugin: starlight-typedoc (and similar) defaults nested sidebar groups to collapsed;
 * force every sidebar group (and autogenerate subgroup) to start expanded.
 */
export function starlightExpandAllSidebarGroups() {
    return {
        name: "starlight-expand-all-sidebar-groups",
        hooks: {
            "config:setup"({
                config,
                updateConfig,
            }: {
                config: { sidebar?: SidebarEntry[] };
                updateConfig: (patch: { sidebar: SidebarEntry[] }) => void;
            }) {
                const { sidebar } = config;
                if (!Array.isArray(sidebar)) {
                    return;
                }

                function expandEntries(
                    entries: SidebarEntry[],
                ): SidebarEntry[] {
                    return entries.map((entry) => expandEntry(entry));
                }

                function expandEntry(entry: SidebarEntry): SidebarEntry {
                    if (typeof entry === "string") {
                        return entry;
                    }
                    if (!entry || typeof entry !== "object") {
                        return entry;
                    }

                    const out: SidebarGroup = { ...entry };
                    if ("collapsed" in out) {
                        out.collapsed = false;
                    }
                    if (
                        out.autogenerate &&
                        typeof out.autogenerate === "object" &&
                        !Array.isArray(out.autogenerate)
                    ) {
                        out.autogenerate = {
                            ...out.autogenerate,
                            collapsed: false,
                        };
                    }
                    if (Array.isArray(out.items)) {
                        out.items = expandEntries(out.items);
                    }
                    return out;
                }

                updateConfig({ sidebar: expandEntries(sidebar) });
            },
        },
    };
}
