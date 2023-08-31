// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

slint::slint!(
import { Button, VerticalBox } from "std-widgets.slint";

export component PreviewUi inherits Window {
    in property<component-factory> preview_area <=> preview_area_container.component-factory;
    callback design_mode_changed(bool);

    VerticalBox {
        design_mode_toggle := Button {
            text: "Design Mode";
            checkable: true;
            clicked => { root.design_mode_changed(self.checked); }
        }
        preview_area_container := ComponentContainer {}
    }
}
);
