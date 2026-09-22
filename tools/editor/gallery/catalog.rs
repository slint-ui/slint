// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub struct Page {
    pub id: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub scenarios: &'static [&'static str],
}

pub const PAGES: &[Page] = &[
    Page {
        id: "foundations",
        title: "Theme and typography",
        description: "Semantic colors, type, spacing, radii, and icons.",
        scenarios: &["Default"],
    },
    Page {
        id: "controls",
        title: "Basic controls",
        description: "Inputs, buttons, segments, and a resize divider.",
        scenarios: &["Default"],
    },
    Page {
        id: "inspector-controls",
        title: "Inspector controls",
        description: "Independent sample values for fields, the slider, and compound controls.",
        scenarios: &["Default"],
    },
    Page {
        id: "palette",
        title: "Element palette",
        description: "Search and expand groups, then drag into the drop target.",
        scenarios: &["Default", "Unavailable", "Dragging disabled"],
    },
    Page {
        id: "picker",
        title: "Color and gradients",
        description: "Edit a sample swatch with the production color and gradient picker.",
        scenarios: &["Default", "Transparent", "Linear", "Radial", "Conic", "Unsupported"],
    },
    Page {
        id: "outline",
        title: "Outline",
        description: "Select and expand sample rows. Drops report their destination.",
        scenarios: &["Default", "Collapsed", "Long names", "Empty", "Unavailable"],
    },
];

pub fn page(id: &str) -> Option<&'static Page> {
    PAGES.iter().find(|p| p.id == id)
}
