// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    slint::platform::set_platform(Box::new(
        i_slint_backend_winit::Backend::new_with_renderer_by_name(Some("skia-opengl")).unwrap(),
    ))
    .unwrap();

    slint::slint! {
        export component App inherits Window {
            Text { text: "Ok"; }
        }
    }

    let app = App::new().unwrap();
    let slint_window = app.window();
    let app_weak = app.as_weak();

    let mut rendered_once = false;
    let screenshot = std::rc::Rc::new(std::cell::RefCell::new(None));

    slint_window
        .set_rendering_notifier({
            let screenshot = screenshot.clone();
            move |state, _| match state {
                slint::RenderingState::BeforeRendering => {
                    if rendered_once {
                        *screenshot.borrow_mut() = Some(app_weak.unwrap().window().grab_window());
                        slint::quit_event_loop().unwrap();
                    }
                }
                slint::RenderingState::AfterRendering => {
                    rendered_once = true;
                    app_weak.unwrap().window().request_redraw();
                }
                _ => {}
            }
        })
        .unwrap();

    app.show().unwrap();
    app.run().unwrap();

    let screenshot = screenshot.borrow_mut().take().unwrap().unwrap();
    assert!(screenshot.width() > 0);
    assert!(screenshot.height() > 0);
}
