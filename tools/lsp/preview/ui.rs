// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use slint_interpreter::PlatformError;

slint::slint!(
import { Button, ScrollView, VerticalBox } from "std-widgets.slint";

export component PreviewUi inherits Window {
    in property<component-factory> preview_area <=> preview_area_container.component-factory;
    callback design_mode_changed(bool);

    preferred-width: 800px;
    preferred-height: 600px;

    VerticalBox {
        design_mode_toggle := Button {
            text: "Design Mode";
            checkable: true;
            clicked => { root.design_mode_changed(self.checked); }
        }

        scroll_area := ScrollView {
            Rectangle {
                width: Math.max(scroll_area.width, preview_area_container.width + 40px);
                height: Math.max(scroll_area.height, preview_area_container.height + 40px);
                background: Colors.red;

                preview_area_container := ComponentContainer {
                    width: self.preferred-width;
                    height: self.preferred-height;
                    x: (parent.width - self.width) / 2;
                    y: (parent.height - self.height) / 2;
                }
            }
        }
    }
}
);

pub fn create_ui() -> Result<PreviewUi, PlatformError> {
    let ui = PreviewUi::new()?;
    ui.on_design_mode_changed(super::set_design_mode);
    Ok(ui)
}
