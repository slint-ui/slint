// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, SoftwareRenderer, TargetPixel,
};
use slint::platform::{PlatformError, WindowAdapter};
use slint::{PhysicalPosition, PhysicalSize};
use std::rc::Rc;

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow>  =
    MinimalSoftwareWindow::new(slint::platform::software_renderer::RepaintBufferType::ReusedBuffer);

}

struct TestPlatform;
impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(WINDOW.with(|x| x.clone()))
    }
}

#[derive(Clone, Copy, Default)]
struct TestPixel(bool);

impl TargetPixel for TestPixel {
    fn blend(&mut self, _color: PremultipliedRgbaColor) {
        *self = Self(true);
    }

    fn from_rgb(_red: u8, _green: u8, _blue: u8) -> Self {
        Self(true)
    }
}

#[track_caller]
fn do_test_render_region(renderer: &SoftwareRenderer, x: i32, y: i32, x2: i32, y2: i32) {
    let mut buffer = vec![TestPixel(false); 500 * 500];
    let r = renderer.render(buffer.as_mut_slice(), 500);
    assert_eq!(r.bounding_box_size(), PhysicalSize { width: (x2 - x) as _, height: (y2 - y) as _ });
    assert_eq!(r.bounding_box_origin(), PhysicalPosition { x, y });

    for py in 0..500 {
        for px in 0..500 {
            assert_eq!(
                buffer[py * 500 + px].0,
                (x..x2).contains(&(px as i32)) && (y..y2).contains(&(py as i32)),
                "unexpected value at {px},{py}"
            )
        }
    }
}

#[test]
fn simple() {
    slint::slint! {
        export component Ui inherits Window {
            in property <color> c: yellow;
            background: black;
            Rectangle {
                x: 1phx;
                y: 80phx;
                width: 15phx;
                height: 17phx;
                background: red;
            }
            Rectangle {
                x: 10phx;
                y: 19phx;
                Rectangle {
                    x: 5phx;
                    y: 80phx;
                    width: 12phx;
                    height: 13phx;
                    background: c;
                }
                Rectangle {
                    x: 50phx;
                    y: 8phx;
                    width: 15phx;
                    height: 17phx;
                    background: c;
                }
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 5, 19 + 8, 10 + 50 + 15, 19 + 80 + 13);
    }));
    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn visibility() {
    slint::slint! {
        export component Ui inherits Window {
            in property <bool> c : true;
            background: black;
            Rectangle {
                x: 10phx;
                y: 19phx;
                Rectangle {
                    x: 5phx;
                    y: 80phx;
                    width: 12phx;
                    height: 13phx;
                    background: red;
                    visible: c;
                }
                Rectangle {
                    x: 50phx;
                    y: 8phx;
                    width: 15phx;
                    height: 17phx;
                    background: gray;
                    visible: !c;
                }
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(false);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 5, 19 + 8, 10 + 50 + 15, 19 + 80 + 13);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(true);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 5, 19 + 8, 10 + 50 + 15, 19 + 80 + 13);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn if_condition() {
    slint::slint! {
        export component Ui inherits Window {
            in property <bool> c : true;
            background: black;
            if c: Rectangle {
                x: 45px;
                y: 45px;
                background: pink;
                width: 32px;
                height: 3px;
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(false);
    assert!(window.draw_if_needed(|renderer| {
        // Currently we redraw when a condition becomes false because we don't track the position otherwise
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_c(true);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 45, 45, 45 + 32, 45 + 3);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}
