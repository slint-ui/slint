// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::ComponentHandle;

mod ui {
    slint::slint! {
        #[edition = "1.20"]
        export { GalleryWindow, Gallery, GalleryPage } from "gallery.slint";
        export { Api, ElementKind, ElementLibraryEntry, ElementLibraryGroup, ElementLibrarySection } from "../ui/api.slint";
        export { Properties, BrushKind, ColorFieldData, ColorFieldEdit, ColorFieldMode, ColorValueData, FillData, GradientStop, GradientStopOrder, LinearGradientAxis, PropertyValue, PropertyValueKind } from "../ui/properties.slint";
    }
}

#[allow(dead_code)]
#[path = "../component_support/mod.rs"]
mod component_support;

use component_support::{brushes, element_library, recent_fills};
use ui::{Api, Gallery, GalleryWindow, Properties};

fn main() -> Result<(), slint::PlatformError> {
    let window = GalleryWindow::new()?;
    let api = window.global::<Api>();
    let properties_api = window.global::<Properties>();
    brushes::setup(&properties_api);
    let gallery =
        <Gallery as slint::Global<'_, GalleryWindow>>::as_weak(&window.global::<Gallery>());
    properties_api.on_hooked_property_value_data(move |_, fallback| {
        let value = gallery.unwrap().get_preview_value();
        if value.value_resolved { value } else { fallback }
    });
    element_library::setup(&api);
    recent_fills::setup(
        &properties_api,
        <Properties as slint::Global<'_, GalleryWindow>>::as_weak(&properties_api),
    );
    window.global::<Gallery>().invoke_navigate(window.global::<Gallery>().get_page(), 0);
    window.run()
}
