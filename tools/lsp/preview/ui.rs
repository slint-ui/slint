// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

slint::slint!(
import { VerticalBox } from "std-widgets.slint";

export component PreviewUi inherits Window {
    in property<component-factory> preview_area <=> cc.component-factory;

    VerticalBox {
       Text { text: "Welcome to the Slint Preview"; }
       cc := ComponentContainer {}
    }
}
);
