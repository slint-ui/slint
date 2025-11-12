// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    slint::slint! {
        import { Palette } from "std-widgets.slint";
        export component App inherits Window {
            property <bool> dark-mode: Palette.color-scheme == ColorScheme.dark;
            MenuBar {
                Menu {
                title: @tr("Settings");
                MenuItem {
                    title: @tr("Toggle dark mode");
                    checkable: true;
                    checked: { Palette.color-scheme == ColorScheme.dark }  // Result in panic "RefCell already mutably borrowed"
                    activated => {
                      Palette.color-scheme = root.dark-mode ? ColorScheme.light : ColorScheme.dark;
                    }
               }
            }
        }
        }
    }
    use slint::winit_030::WinitWindowAccessor;
    slint::BackendSelector::new().backend_name("winit".into()).select().unwrap();
    slint::spawn_local(async move {
        let app = App::new().unwrap();
        let slint_window = app.window();

        app.show().unwrap();
        let result = slint_window.winit_window().await;
        assert!(result.is_ok(), "Failed to get winit window: {:?}", result.err());
        slint::quit_event_loop().unwrap();
    })
    .unwrap();

    slint::run_event_loop().unwrap();
}
