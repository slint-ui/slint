// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: MIT

pub mod backend {
    use slint::{Model, SharedString};

    slint::include_modules!();

    pub fn init(blogica_api: &BLogicAAPI) {
        blogica_api.set_code1(SharedString::from("Important thing"));
        blogica_api.set_code2(SharedString::from("Another important thing"));
        blogica_api.set_code3(SharedString::from("Yet another important thing"));
        blogica_api.set_code4(SharedString::from("One more important thing"));

        blogica_api.on_update({
            let blogica_api = blogica_api.as_weak();
            move |bdata| {
                {
                    let blogica_api = blogica_api.upgrade().unwrap();

                    if bdata.colors.row_count() >= 4 {
                        blogica_api.set_color1(bdata.colors.row_data(0).unwrap());
                        blogica_api.set_color2(bdata.colors.row_data(1).unwrap());
                        blogica_api.set_color3(bdata.colors.row_data(2).unwrap());
                        blogica_api.set_color4(bdata.colors.row_data(3).unwrap());
                    }
                }

                blogica_api.upgrade_in(move |blogica_api| {
                    if bdata.codes.row_count() >= 4 {
                        blogica_api.set_code1(bdata.codes.row_data(0).unwrap());
                        blogica_api.set_code2(bdata.codes.row_data(1).unwrap());
                        blogica_api.set_code3(bdata.codes.row_data(2).unwrap());
                        blogica_api.set_code4(bdata.codes.row_data(3).unwrap());
                    }
                });
            }
        });

        blogica_api.set_initialized(true);
    }
}
