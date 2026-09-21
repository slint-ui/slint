// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::winit_030::WinitWindowAccessor;

slint::slint! {
    import { ScrollView } from "std-widgets.slint";
    export component Comparison inherits Window {
        title: "UIKit over Slint";
        background: #f4f6fa;
        out property <float> scroll-offset: -list.content-y / 1px;
        callback set-scroll-offset(float);
        set-scroll-offset(offset) => { list.content-y = -offset * 1px; }
        list := ScrollView {
            x: 8px; y: 102px;
            width: root.width - 16px; height: root.height - 148px;
            content-width: self.width - 12px; content-height: 1000 * 72px;
            horizontal-scrollbar-policy: ScrollBarPolicy.always-off;
            for row in 1000 : Rectangle {
                y: row * 72px; height: 72px; width: list.content-width;
                background: mod(row, 2) == 0 ? #e4ebf5 : #ffffff;
                Text {
                    x: 12px; height: parent.height; vertical-alignment: center;
                    text: "Slint " + (row + 1); font-size: 16px; color: #145aaa;
                }
            }
        }
    }
}

thread_local! {
    static APP: std::cell::RefCell<Option<slint::Weak<Comparison>>> = const { std::cell::RefCell::new(None) };
}

#[unsafe(no_mangle)]
extern "C" fn slint_scroll_offset() -> f32 {
    APP.with(|slot| {
        slot.borrow()
            .as_ref()
            .and_then(slint::Weak::upgrade)
            .map_or(0.0, |app| app.get_scroll_offset())
    })
}

#[unsafe(no_mangle)]
extern "C" fn set_slint_scroll_offset(offset: f32) {
    APP.with(|slot| {
        if let Some(app) = slot.borrow().as_ref().and_then(slint::Weak::upgrade) {
            app.invoke_set_scroll_offset(offset);
        }
    });
}

unsafe extern "C" {
    fn install_native_scroll(host: *mut std::ffi::c_void);
}

fn main() {
    let app = Comparison::new().unwrap();
    APP.with(|slot| *slot.borrow_mut() = Some(app.as_weak()));
    let weak = app.as_weak();
    slint::spawn_local(async move {
        let app = weak.unwrap();
        let window = app.window().winit_window().await.unwrap();
        let RawWindowHandle::UiKit(handle) = window.window_handle().unwrap().as_raw() else {
            panic!("This comparison requires iOS");
        };
        unsafe { install_native_scroll(handle.ui_view.as_ptr()) };
    })
    .unwrap();
    app.run().unwrap();
}
