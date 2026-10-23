// Copyright © 2026 Koichiro Ohba
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Koichiro Ohba

#[cfg(target_os = "macos")]
fn main() {
    use slint::ComponentHandle;
    use slint::winit_030::WinitWindowAccessor;
    use std::cell::Cell;
    use std::rc::Rc;

    slint::slint! {
        export component App inherits Window {
            width: 100px;
            height: 100px;
            in property <color> fill: red;
            background: root.fill;
        }
    }

    slint::BackendSelector::new()
        .backend_name("winit".into())
        .renderer_name("femtovg".into())
        .select()
        .unwrap();

    slint::spawn_local(async move {
        let app = App::new().unwrap();
        let hidden_frames = Rc::new(Cell::new(0));
        let frames = hidden_frames.clone();
        let weak = app.as_weak();
        app.window()
            .set_rendering_notifier(move |state, _| {
                if matches!(state, slint::RenderingState::AfterRendering) {
                    let app = weak.unwrap();
                    if app.window().with_winit_window(|w| w.is_visible()) == Some(Some(false)) {
                        frames.set(frames.get() + 1);
                    }
                }
            })
            .unwrap();

        app.show().unwrap();
        let _ = app.window().winit_window().await.unwrap();
        assert!(hidden_frames.get() > 0, "the first show did not render before mapping");

        for color in [slint::Color::from_rgb_u8(0, 255, 0), slint::Color::from_rgb_u8(0, 0, 255)] {
            app.hide().unwrap();
            app.set_fill(color);
            let before = hidden_frames.get();
            app.show().unwrap();
            assert!(
                hidden_frames.get() > before,
                "show reused the old frame instead of rendering before mapping"
            );
        }
        app.hide().unwrap();
        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop_until_quit().unwrap();
}

#[cfg(not(target_os = "macos"))]
fn main() {}
