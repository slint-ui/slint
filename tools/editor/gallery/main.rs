// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::ComponentHandle;
use slint_editor::{
    component_support::{brushes, element_library, recent_fills},
    ui::{Api, Gallery, GalleryWindow},
};

fn main() -> Result<(), slint::PlatformError> {
    let window = GalleryWindow::new()?;
    let api = window.global::<Api>();
    brushes::setup(&api);
    element_library::setup(&api);
    recent_fills::setup(&api, <Api as slint::Global<'_, GalleryWindow>>::as_weak(&api));
    window.global::<Gallery>().invoke_navigate(window.global::<Gallery>().get_page_index(), 0);
    window.run()
}
