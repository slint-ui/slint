// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

pub trait CreateTaskController {
    fn refresh(&self);
    fn back(&self);
}
