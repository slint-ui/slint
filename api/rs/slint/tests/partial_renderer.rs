// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, SoftwareRenderer, TargetPixel,
};
use slint::platform::{PlatformError, WindowAdapter};
use slint::{Model, PhysicalPosition, PhysicalSize};
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

    let mut has_one_pixel = false;
    for py in 0..500 {
        for px in 0..500 {
            let in_bounding_box = (x..x2).contains(&(px as i32)) && (y..y2).contains(&(py as i32));
            if !in_bounding_box {
                assert!(!buffer[py * 500 + px].0, "Something written outside of bounding box in  {px},{py}   - (x={x},y={y},x2={x2},y2={y2})")
            } else if buffer[py * 500 + px].0 {
                has_one_pixel = true;
            }
        }
    }
    assert!(has_one_pixel, "Nothing was rendered");
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

#[test]
fn list_view() {
    slint::slint! {
        // We can't rely on the style as they are all different so implement our own in very basic terms
        component ListView inherits Flickable {
            out property <length> visible-width <=> self.width;
            out property <length> visible-height <=> self.height;
            @children
        }
        export component Ui inherits Window {
            width: 300px;
            height: 300px;
            in property <[int]> model;
            ListView {
                x: 20px; y: 10px; width: 100px; height: 90px;
                for x in model: Rectangle {
                    background: x == 1 ? red : blue;
                    height: 10px;
                    width: 25px;
                }

            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(300, 300));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 300, 300);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    let model = std::rc::Rc::new(slint::VecModel::from(vec![0]));
    ui.set_model(model.clone().into());

    const LV_X: i32 = 20;
    const LV_Y: i32 = 10;

    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, LV_X, LV_Y, LV_X + 25, LV_Y + 10);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    model.insert(0, 1);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, LV_X, LV_Y, LV_X + 25, LV_Y + 20);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    model.set_row_data(1, 1);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, LV_X, LV_Y + 10, LV_X + 25, LV_Y + 20);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    model.set_vec(vec![0, 0]);
    assert!(window.draw_if_needed(|renderer| {
        // Currently, when ItemTree are removed, we redraw the whole window.
        do_test_render_region(renderer, 0, 0, 300, 300);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    model.remove(1);
    assert!(window.draw_if_needed(|renderer| {
        // Currently, when ItemTree are removed, we redraw the whole window.
        do_test_render_region(renderer, 0, 0, 300, 300);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
/// test for #6932
fn scale_factor() {
    slint::slint! {
        export component Ui inherits Window {
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(500, 500));
    window.dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor: 1.33 });
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 500, 500);
    }));
}
