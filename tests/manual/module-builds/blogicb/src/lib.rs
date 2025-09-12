// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::SharedString;

slint::include_modules!();

pub fn init(blogicb_api: &BLogicBAPI) {
    blogicb_api.set_crank1(SharedString::from("1"));
    blogicb_api.set_crank2(SharedString::from("2"));
    blogicb_api.set_crank3(SharedString::from("3"));
    blogicb_api.set_crank4(SharedString::from("5"));
    blogicb_api.set_crank5(SharedString::from("7"));
    blogicb_api.set_crank6(SharedString::from("11"));

    // TODO: if BLogicBAPI can be a shared reference, so we can connect callbacks here
    // and pass / move the reference to the closures

    blogicb_api.set_initialized(true);
}
