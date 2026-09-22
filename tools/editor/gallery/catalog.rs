// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub struct Page {
    pub id: &'static str,
    pub category: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub scenarios: &'static [&'static str],
}

pub const PAGES: &[Page] = &[
    Page {
        id: "composition",
        category: "Editor",
        title: "Editor playground",
        description: "Drag elements, select objects and edit their properties. All changes are local.",
        scenarios: &["Default", "Empty", "Long names", "Read only"],
    },
    Page {
        id: "foundations",
        category: "Foundations",
        title: "Theme and typography",
        description: "The editor's semantic colors, type, spacing and element icons.",
        scenarios: &["Default"],
    },
    Page {
        id: "controls",
        category: "Controls",
        title: "Basic controls",
        description: "Production inputs, buttons, segments and resize dividers.",
        scenarios: &["Default", "Disabled", "Long names"],
    },
    Page {
        id: "inspector-controls",
        category: "Controls",
        title: "Inspector controls",
        description: "Fields, sliders, rotation and corner controls with editable fixture values.",
        scenarios: &["Default", "Disabled", "Rejected edits"],
    },
    Page {
        id: "palette",
        category: "Panels",
        title: "Element palette",
        description: "Search and expand groups, then drag an element into the sample canvas.",
        scenarios: &["Default", "Unavailable", "Dragging disabled"],
    },
    Page {
        id: "picker",
        category: "Panels",
        title: "Color and gradients",
        description: "The real fill session: color, alpha, stops, geometry, commit and cancel.",
        scenarios: &[
            "Default",
            "Transparent",
            "Linear",
            "Radial",
            "Conic",
            "Hard edge",
            "Unsupported",
            "Rejected edits",
        ],
    },
    Page {
        id: "outline",
        category: "Panels",
        title: "Outline",
        description: "Select, expand, reorder and reparent a local hierarchy. Invalid drops are rejected.",
        scenarios: &["Default", "Collapsed", "Deep tree", "Long names", "Empty", "Unavailable"],
    },
    Page {
        id: "files",
        category: "Panels",
        title: "File tree",
        description: "Browse and rename sample files without touching the filesystem.",
        scenarios: &["Default", "Collapsed", "Long names", "Empty"],
    },
    Page {
        id: "inspector",
        category: "Panels",
        title: "Inspector",
        description: "The full property inspector and outline, driven by a sample selection.",
        scenarios: &["Default", "Text", "Image", "TouchArea", "No selection", "Read only"],
    },
    Page {
        id: "canvas",
        category: "Canvas",
        title: "Canvas tools",
        description: "Production selection, resize, rotation, radius and gradient handles.",
        scenarios: &["Default", "Rotated", "Linear", "Radial", "Conic", "Clipped", "Disabled"],
    },
    Page {
        id: "images",
        category: "Canvas",
        title: "Image asset editor",
        description: "Preview a bundled asset and adjust nine-slice guides.",
        scenarios: &["Default", "Raster", "Missing image"],
    },
    Page {
        id: "welcome",
        category: "Application",
        title: "Startup screen",
        description: "Recent projects and project actions use local fixtures.",
        scenarios: &["Default", "Empty", "Long names"],
    },
    Page {
        id: "shell",
        category: "Application",
        title: "Toolbar and status",
        description: "Run controls, file errors and every updater state. Actions are simulated.",
        scenarios: &[
            "Default",
            "Update available",
            "Downloading",
            "Ready to install",
            "Installing",
            "Restart required",
            "Update failed",
        ],
    },
];

pub fn page(id: &str) -> Option<&'static Page> {
    PAGES.iter().find(|p| p.id == id)
}
