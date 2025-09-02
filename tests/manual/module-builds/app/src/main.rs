// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use blogica;
use blogicb;
use random_word;
use std::error::Error;

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
    let ui = AppWindow::new()?;

    let blociga_api = ui.global::<blogica::backend::BLogicAAPI>();
    blogica::backend::init(&blociga_api);

    let blogicb_api = ui.global::<blogicb::BLogicBAPI>();
    blogicb::init(&blogicb_api);

    ui.on_update_blogic_data({
        let ui_handle = ui.as_weak();
        move || {
            let ui = ui_handle.upgrade().unwrap();
            let blogica_api = ui.global::<blogica::backend::BLogicAAPI>();
            let mut bdata = blogica::backend::BData::default();

            bdata.colors = slint::ModelRc::new(slint::VecModel::from(
                (1..6)
                    .into_iter()
                    .map(|_| {
                        let red = rand::random::<u8>();
                        let green = rand::random::<u8>();
                        let blue = rand::random::<u8>();
                        slint::Color::from_rgb_u8(red, green, blue)
                    })
                    .collect::<Vec<_>>(),
            ));

            bdata.codes = slint::ModelRc::new(slint::VecModel::from(
                (1..6)
                    .into_iter()
                    .map(|_| slint::SharedString::from(random_word::get(random_word::Lang::En)))
                    .collect::<Vec<_>>(),
            ));

            blogica_api.invoke_update(bdata);
        }
    });

    ui.run()?;

    Ok(())
}
