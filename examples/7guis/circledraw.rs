// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use slint::VecModel;
use std::rc::Rc;

slint::slint!(import { MainWindow } from "circledraw.slint";);

pub fn main() {
    let main_window = MainWindow::new();

    let model = Rc::new(VecModel::default());
    main_window.set_model(model.clone().into());
    main_window.on_background_clicked(move |x, y| {
        model.push(Circle { x: x as f32, y: y as f32, d: 30.0 })
    });
    main_window.run();
}
