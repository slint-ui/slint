// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::cell::Cell;

type CallbackWrapper<Arguments, Result = ()> =
    Cell<Option<Box<dyn FnMut(&Arguments, &mut Result)>>>;

pub struct Callback<Arguments: ?Sized, Result = ()> {
    callback: CallbackWrapper<Arguments, Result>,
}

impl<Arguments: ?Sized, Res> Default for Callback<Arguments, Res> {
    fn default() -> Self {
        Self { callback: Default::default() }
    }
}

impl<Arguments: ?Sized, Result: Default> Callback<Arguments, Result> {
    pub fn on(&self, mut f: impl FnMut(&Arguments) -> Result + 'static) {
        self.callback.set(Some(Box::new(move |a: &Arguments, r: &mut Result| *r = f(a))));
    }

    pub fn invoke(&self, a: &Arguments) -> Result {
        let mut result = Result::default();

        if let Some(mut callback) = self.callback.take() {
            callback(a, &mut result);
            self.callback.set(Some(callback));
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_invoke() {
        let callback: Callback<(i32, i32), i32> = Callback::default();
        callback.on(|(a, b)| a + b);
        assert_eq!(callback.invoke(&(3, 2)), 5);
    }
}
