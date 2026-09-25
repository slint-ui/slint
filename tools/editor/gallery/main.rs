// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::ComponentHandle;

mod ui {
    slint::slint! {
        export { GalleryWindow, Gallery, GalleryPage } from "gallery.slint";
        export {
            Api, BrushKind, ColorData, ElementKind, ElementLibraryEntry,
            ElementLibraryGroup, FillData, GradientStop, GradientStopOrder,
            LinearGradientAxis
        } from "../ui/api.slint";
    }
}

#[allow(dead_code)]
#[path = "../component_support/mod.rs"]
mod component_support;

use component_support::{brushes, element_library, recent_fills};
use ui::{Api, Gallery, GalleryWindow};

fn main() -> Result<(), slint::PlatformError> {
    let window = GalleryWindow::new()?;
    let api = window.global::<Api>();
    brushes::setup(&api);
    element_library::setup(&api);
    recent_fills::setup(&api, <Api as slint::Global<'_, GalleryWindow>>::as_weak(&api));
    window.global::<Gallery>().invoke_navigate(window.global::<Gallery>().get_page(), 0);
    window.run()
}
