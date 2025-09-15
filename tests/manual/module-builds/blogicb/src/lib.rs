// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: MIT

use slint::{Model, SharedString};

slint::include_modules!();

pub fn init(blogicb_api: &BLogicBAPI) {
    blogicb_api.set_crank1(SharedString::from("1"));
    blogicb_api.set_crank2(SharedString::from("2"));
    blogicb_api.set_crank3(SharedString::from("3"));
    blogicb_api.set_crank4(SharedString::from("5"));
    blogicb_api.set_crank5(SharedString::from("7"));
    blogicb_api.set_crank6(SharedString::from("11"));

    blogicb_api.on_crank_it({
        let blogicb_api = blogicb_api.as_weak();
        move |crank_data| {
            {
                let blogicb_api = blogicb_api.upgrade().unwrap();

                if crank_data.cranks.row_count() >= 6 {
                    blogicb_api.set_crank1(crank_data.cranks.row_data(0).unwrap());
                    blogicb_api.set_crank2(crank_data.cranks.row_data(1).unwrap());
                    blogicb_api.set_crank3(crank_data.cranks.row_data(2).unwrap());
                    blogicb_api.set_crank4(crank_data.cranks.row_data(3).unwrap());
                    blogicb_api.set_crank5(crank_data.cranks.row_data(4).unwrap());
                    blogicb_api.set_crank6(crank_data.cranks.row_data(5).unwrap());
                }
            }

            std::thread::spawn({
                let blogicb_api = blogicb_api.clone();
                let magic_number = crank_data.magic_number;
                move || {
                    blogicb_api
                        .upgrade_in_event_loop(move |blogicb_api| {
                            if magic_number == 42 {
                                blogicb_api.set_status(SharedString::from(
                                    "The answer to life, the universe and everything",
                                ));
                            } else {
                                blogicb_api.set_status(SharedString::from("Just a regular number"));
                            }
                        })
                        .ok();
                }
            });
        }
    });

    blogicb_api.set_initialized(true);
}
