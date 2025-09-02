// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub mod backend {
    use slint::SharedString;

    slint::include_modules!();

    pub fn init(blogica_api: &BLogicAAPI) {
        blogica_api.set_code1(SharedString::from("Important thing"));
        blogica_api.set_code2(SharedString::from("Another important thing"));
        blogica_api.set_code3(SharedString::from("Yet another important thing"));
        blogica_api.set_code4(SharedString::from("One more important thing"));

        blogica_api.set_initialized(true);
    }
}
