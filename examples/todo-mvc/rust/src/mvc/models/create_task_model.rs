// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use super::{DateModel, TimeModel};

pub struct CreateTaskModel {
    pub title: String,
    pub due_data: DateModel,
    pub due_time: TimeModel,
}
