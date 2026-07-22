// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

slint::slint! {

import { VerticalBox, CheckBox, LineEdit, Spinner } from "std-widgets.slint";

export component MainWindow inherits Window {
    callback toggle-visibility(bool);
    callback toggle-minimized(bool);
    callback toggle-maximized(bool);
    callback set-title(string);

    VerticalBox {
        CheckBox {
            text: "visible";
            checked: true;
            toggled => { root.toggle-visibility(self.checked); }
        }
        CheckBox {
            text: "minimized";
            checked: false;
            toggled => { root.toggle-minimized(self.checked); }
        }
        CheckBox {
            text: "maximized";
            checked: false;
            toggled => { root.toggle-maximized(self.checked); }
        }
        Text {
            text: "title:";
        }
        LineEdit {
            accepted => { root.set-title(self.text); }
        }
    }
}

export component TestWindow inherits Window {
    in property<string> window-title <=> self.title;
    VerticalBox {
        Text {
            text: "Go ahead and use the\nother window to control aspects\nof this window.";
        }
        Spinner {
            indeterminate: true;
        }
    }
}

}

fn main() -> Result<(), slint::PlatformError> {
    let main_window = MainWindow::new()?;
    main_window.show()?;

    let test_window = TestWindow::new()?;
    test_window.show()?;

    main_window.on_toggle_visibility({
        let test_window_weak = test_window.as_weak();
        move |visible| {
            let Some(test_window) = test_window_weak.upgrade() else { return };
            if visible {
                test_window.show().unwrap();
            } else {
                test_window.hide().unwrap()
            }
        }
    });

    main_window.on_toggle_minimized({
        let test_window_weak = test_window.as_weak();
        move |minimized| {
            let Some(test_window) = test_window_weak.upgrade() else { return };
            test_window.window().set_minimized(minimized);
        }
    });

    main_window.on_toggle_maximized({
        let test_window_weak = test_window.as_weak();
        move |maximized| {
            let Some(test_window) = test_window_weak.upgrade() else { return };
            test_window.window().set_maximized(maximized);
        }
    });

    main_window.on_set_title({
        let test_window_weak = test_window.as_weak();
        move |title| {
            let Some(test_window) = test_window_weak.upgrade() else { return };
            test_window.set_window_title(title);
        }
    });

    slint::run_event_loop()
}
