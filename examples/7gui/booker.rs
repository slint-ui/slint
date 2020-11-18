/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use sixtyfps::Model;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

sixtyfps::sixtyfps!(import { Booker } from "booker.60";);

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    let booker = Booker::new();
    booker.run();
}
