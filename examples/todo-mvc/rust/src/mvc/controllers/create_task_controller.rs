// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::rc::Rc;

pub struct CreateTaskControllerCallbacks {
    pub on_refresh: Box<dyn Fn()>,
    pub on_back: Box<dyn Fn()>,
}

pub struct CreateTaskController {
    callbacks: CreateTaskControllerCallbacks,
}

impl CreateTaskController {
    pub fn new(callbacks: CreateTaskControllerCallbacks) -> Rc<Self> {
        Rc::new(Self { callbacks })
    }

    pub fn refresh(&self) {
        (self.callbacks.on_refresh)();
    }

    pub fn back(&self) {
        (self.callbacks.on_back)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn test_back() {
        let callback_invoked = Rc::new(Cell::new(false));

        let controller = CreateTaskController::new(CreateTaskControllerCallbacks {
            on_refresh: Box::new(|| {}),
            on_back: Box::new({
                let callback_invoked = callback_invoked.clone();

                move || {
                    callback_invoked.set(true);
                }
            }),
        });

        controller.back();

        assert!(callback_invoked.get());
    }
}
