// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore doesnt

//! Tests of the partial renderer with the software renderer.

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
                assert!(
                    !buffer[py * 500 + px].0,
                    "Something written outside of bounding box in  {px},{py}   - (x={x},y={y},x2={x2},y2={y2})"
                )
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
        #[edition = "1.19"]
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
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <bool> c : true;
            in property <bool> another: false;
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
                    Rectangle {
                        x: 3phx;
                        y: 3phx;
                        width: 3phx;
                        height: 3phx;
                        background: another ? blue: green;
                    }
                }
                Rectangle {
                    x: 50phx;
                    y: 8phx;
                    width: 15phx;
                    height: 17phx;
                    background: gray;
                    visible: !c;
                    Rectangle {
                        x: 3phx;
                        y: 3phx;
                        width: 3phx;
                        height: 3phx;
                        background: another ? blue: green;
                    }
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
    ui.set_another(true);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 50 + 3, 19 + 8 + 3, 10 + 50 + 6, 19 + 8 + 6);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    ui.set_c(true);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 5, 19 + 8, 10 + 50 + 15, 19 + 80 + 13);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_another(false);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 5 + 3, 19 + 80 + 3, 10 + 5 + 6, 19 + 80 + 6);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn if_condition() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <bool> c : true;
            background: black;
            // Static siblings before and after the conditional: toggling the condition
            // must not repaint the one before. The one after currently repaints when the
            // condition turns false: the disappearance shifts its rank among the visited
            // siblings, which is indistinguishable from a z-order change (see the
            // `sibling_index` comment in `CachedItemBoundingBoxAndTransform`).
            Rectangle { x: 5px; y: 5px; width: 8px; height: 8px; background: green; }
            if c: Rectangle {
                x: 45px;
                y: 45px;
                background: pink;
                width: 32px;
                height: 3px;
            }
            Rectangle { x: 100px; y: 200px; width: 20px; height: 20px; background: blue; }
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
        // The vanished conditional plus the sibling after it (see the comment above).
        do_test_render_region(renderer, 45, 45, 100 + 20, 200 + 20);
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
        #[edition = "1.19"]
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
        // The model reset drops and recreates both rows.
        do_test_render_region(renderer, LV_X, LV_Y, LV_X + 25, LV_Y + 20);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    model.remove(1);
    assert!(window.draw_if_needed(|renderer| {
        // Only the region of the removed row repaints.
        do_test_render_region(renderer, LV_X, LV_Y + 10, LV_X + 25, LV_Y + 20);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    model.set_vec(vec![1, 0]);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, LV_X, LV_Y, LV_X + 25, LV_Y + 20);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    model.remove(0);
    assert!(window.draw_if_needed(|renderer| {
        // Removing a leading row repaints the following rows too: they move up.
        do_test_render_region(renderer, LV_X, LV_Y, LV_X + 25, LV_Y + 20);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
/// test for #6932
fn scale_factor() {
    slint::slint! {
        #[edition = "1.19"]
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

#[test]
fn window_background() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <color> c: yellow;
            background: c;
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
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    ui.set_c(slint::Color::from_rgb_u8(45, 12, 13));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn touch_area_doesnt_cause_redraw() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <color> c: yellow;
            in property <length> touch-area-1-x <=> ta1.x;
            in property <length> touch-area-2-x <=> ta2.x;
            in property <color> sole-pixel-color: red;
            background: black;
            ta1 := TouchArea {
                x: 10px;
                y: 0px;
                width: 20px;
                height: 40px;
                Rectangle {
                    x: 1phx;
                    y: 20phx;
                    width: 15phx;
                    height: 17phx;
                    background: c;
                }
            }
            ta2 := TouchArea {
                x: 10px;
                y: 0px;
                width: 20px;
                height: 40px;
            }
            sole-pixel := Rectangle {
                x: 60px;
                y: 0px;
                width: 1px;
                height: 1px;
                background: sole-pixel-color;
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
        do_test_render_region(renderer, 10 + 1, 20, 10 + 1 + 15, 20 + 17);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_touch_area_1_x(20.);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10 + 1, 20, 10 + 1 + 15 + 10, 20 + 17);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_touch_area_2_x(20.);
    ui.set_sole_pixel_color(slint::Color::from_rgb_u8(45, 12, 13));
    // Moving the touch area should not cause it to be redrawn.
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 60, 0, 61, 1);
    }));
}

#[test]
/// Regression test for #12173: when an item is rendering-dirty (here a color
/// change) but keeps its geometry while its parent moves, the old position must
/// still be cleared. The child is wider than its parent, so its protruding part
/// is not covered by the parent's repaint and would otherwise leave a ghost.
fn moving_rendering_dirty_item_clears_old_position() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <length> pos;
            in property <color> c: red;
            background: black;
            Rectangle {
                x: root.pos;
                y: 50px;
                width: 20px;
                height: 20px;
                background: skyblue;
                Rectangle {
                    // Same geometry every frame, but wider than the parent so
                    // it protrudes onto the window background.
                    x: 0px;
                    y: 0px;
                    width: 100px;
                    height: 10px;
                    background: root.c;
                }
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(400, 200));
    ui.set_pos(200.);
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 400, 200);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    // Move the rectangle left and recolor the child. The color change makes the
    // child rendering-dirty; the move changes its transform. The dirty region
    // must reach the child's old right edge (200 + 100 = 300), not stop at the
    // parent's old geometry (220).
    ui.set_pos(20.);
    ui.set_c(slint::Color::from_rgb_u8(0, 0, 255));
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 20, 50, 300, 70);
    }));
}

#[test]
/// Two overlapping siblings swapping their dynamic z order must repaint the overlap,
/// even though nothing but the stacking order changed. See #12397.
fn dynamic_z_order() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <int> z-red: 1;
            in property <int> z-blue: 2;
            background: black;
            // Two fully overlapping rectangles: only the z order decides which is visible.
            Rectangle {
                x: 10phx;
                y: 20phx;
                width: 30phx;
                height: 40phx;
                background: red;
                z: root.z-red;
            }
            Rectangle {
                x: 10phx;
                y: 20phx;
                width: 30phx;
                height: 40phx;
                background: blue;
                z: root.z-blue;
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

    // Bring the red rectangle on top. Nothing moved, but the overlap must be
    // repainted because the visible pixels there change from blue to red.
    ui.set_z_red(3);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10, 20, 10 + 30, 20 + 40);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    // Swapping back must repaint the overlap again.
    ui.set_z_red(1);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10, 20, 10 + 30, 20 + 40);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

/// Assert that drawing produces no repaint: either no redraw is requested, or the
/// dirty region is empty.
#[track_caller]
fn do_test_no_repaint(window: &MinimalSoftwareWindow) {
    window.draw_if_needed(|renderer| {
        let mut buffer = vec![TestPixel(false); 500 * 500];
        let r = renderer.render(buffer.as_mut_slice(), 500);
        assert_eq!(r.bounding_box_size(), PhysicalSize::default(), "expected no repaint");
        assert!(buffer.iter().all(|p| !p.0), "expected no pixel to be written");
    });
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
/// Like `dynamic_z_order`, but the reordered siblings are `for` instances whose z comes
/// from the model. Two spatially separate clusters (kept apart in z so they never
/// interleave) check that only the reordered cluster repaints, that moving an instance
/// across several others repaints, and that a z change preserving the order repaints nothing.
fn dynamic_z_order_repeater() {
    slint::slint! {
        #[edition = "1.19"]
        struct ZItem { px: length, py: length, z: int }
        export component Ui inherits Window {
            in property <[ZItem]> items;
            background: black;
            for it in items: Rectangle {
                x: it.px;
                y: it.py;
                width: 40phx;
                height: 40phx;
                background: red;
                z: it.z;
            }
        }
    }

    // Cluster A stacked at (10,20), cluster B at (150,150).
    let items = std::rc::Rc::new(slint::VecModel::from(vec![
        ZItem { px: 10., py: 20., z: 10 },
        ZItem { px: 10., py: 20., z: 20 },
        ZItem { px: 10., py: 20., z: 30 },
        ZItem { px: 150., py: 150., z: 110 },
        ZItem { px: 150., py: 150., z: 120 },
        ZItem { px: 150., py: 150., z: 130 },
    ]));

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    ui.set_items(items.clone().into());
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(300, 300));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 300, 300);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    let set_z = |i: usize, z: i32| {
        let mut it = items.row_data(i).unwrap();
        it.z = z;
        items.set_row_data(i, it);
    };

    // Reorder inside cluster A (instance 0 rises above instance 1): only cluster A repaints.
    set_z(0, 25);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10, 20, 10 + 40, 20 + 40);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    // A z change that keeps the order (still between 20 and 30): nothing repaints.
    set_z(0, 27);
    do_test_no_repaint(&window);

    // Reorder inside cluster B: only cluster B repaints, cluster A stays clean.
    set_z(3, 125);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 150, 150, 150 + 40, 150 + 40);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    // Move instance 0 across both other cluster-A instances to the top: cluster A repaints.
    set_z(0, 35);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10, 20, 10 + 40, 20 + 40);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
/// Reversing the z order of three siblings in one frame must repaint the overlap of
/// every pair whose relative order flipped — including a pair where neither member's
/// rank *decreased*: here B keeps its middle rank and A only rises, yet A slides on
/// top of B, so the A∩B overlap must be repainted (via A's rect).
fn dynamic_z_order_reversal() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <bool> rev: false;
            background: black;
            // A and B overlap each other; C is far away from both.
            Rectangle { x: 10phx; y: 10phx; width: 30phx; height: 30phx; background: red; z: rev ? 30 : 10; } // A
            Rectangle { x: 25phx; y: 25phx; width: 30phx; height: 30phx; background: blue; z: 20; } // B
            Rectangle { x: 150phx; y: 150phx; width: 30phx; height: 30phx; background: green; z: rev ? 10 : 30; } // C
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

    // Reverse the stacking order: A (rank 0→2) and C (rank 2→0) change rank, B stays
    // at rank 1. The dirty region is A ∪ C, which covers both flipped overlaps
    // (A∩B is within A, B∩C within C).
    ui.set_rev(true);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10, 10, 180, 180);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    // And back.
    ui.set_rev(false);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 10, 10, 180, 180);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
/// A sibling appearing in the same frame as a z swap must not mask the swap: the
/// conditional element enters the stacking order below everything, shifting the
/// following ranks up, so the demoted rectangle's rank does not decrease — but the
/// overlap of the swapped pair must still be repainted.
fn dynamic_z_order_swap_with_appearing_sibling() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <bool> front: false;
            background: black;
            // Appears below everything, away from the overlapping pair.
            if front: Rectangle { x: 140phx; y: 140phx; width: 8phx; height: 8phx; background: green; z: -1; }
            Rectangle { x: 20phx; y: 20phx; width: 30phx; height: 30phx; background: red; z: front ? 0 : 10; }
            Rectangle { x: 20phx; y: 20phx; width: 30phx; height: 30phx; background: blue; z: 5; }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(200, 200));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 200, 200);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    // One property flip shows the green rectangle and swaps red below blue.
    // The dirty region is the swapped pair's rect plus the appearing rectangle.
    ui.set_front(true);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 20, 20, 148, 148);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn nowrap_text_change_doesnt_change_height() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <string> first-text: "First text";
            out property <length> first-label-width: first-label.width;
            out property <length> first-label-height: first-label.height;
            background: black;
            VerticalLayout {
                first-label := Text {
                    text: root.first-text;
                }
                Text {
                    text: "Second text";
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
    ui.set_first_text("Hello World longer".into());

    let expected_width = ui.get_first_label_width().ceil() as _;
    let expected_height = ui.get_first_label_height().ceil() as _;

    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, expected_width, expected_height);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn create_item_tree_during_rendering() {
    // This test has `init` callbacks that cascade: cond1's init sets cond2=true,
    // cond2-red's init sets cond3=true. The ensure_tree_instantiated loop
    // materializes all three levels before rendering, so every rectangle
    // lands in the first draw's dirty region.
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <bool> cond1: false;
            property <bool> cond2: false;
            property <bool> cond3: false;

            in property <length> foo: 5px;

            if cond3: Rectangle {
                x: 12px;
                y: foo;
                width: 10px;
                height: 10px;
                background: yellow;
            }
            if cond2: Rectangle {
                x: 10px;
                y: 10px;
                width: 10px;
                height: 10px;
                background: red;
                init => { cond3=true; }
            }
            if cond1: Rectangle {
                x: 10px;
                y: 15px;
                width: 10px;
                height: 10px;
                background: blue;
                init => { cond2=true; }
            }
            if cond2: Rectangle {
                x: 12px;
                y: 10px + foo;
                width: 10px;
                height: 10px;
                background: green;
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
    ui.set_cond1(true);

    assert!(window.draw_if_needed(|renderer| {
        // All three cascaded conditionals are instantiated before rendering:
        // cond3's rect is at y=5 (foo), so the region starts at y=5.
        do_test_render_region(renderer, 10, 5, 22, 25);
    }));

    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    ui.set_foo(4.0);

    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 12, 4, 22, 25);
    }));

    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    ui.set_foo(3.0);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 12, 3, 22, 24);
    }));
}

#[test]
fn init_property_read_does_not_trigger_redraw() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            width: 100px;
            height: 100px;

            in property <bool> cond: false;
            in property <length> unrelated: 10px;
            property <length> stash;

            if cond: Rectangle {
                x: 5px;
                y: 5px;
                width: 20px;
                height: 20px;
                background: red;
                // The init callback reads `unrelated`.  That read must NOT
                // register as a dependency of the redraw tracker.
                init => { stash = unrelated; }
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = Ui::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(100, 100));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 100, 100);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    // Activate the conditional — the rectangle appears and init runs.
    ui.set_cond(true);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 5, 5, 25, 25);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    // Change the property that the init callback read.
    // This must NOT trigger a redraw because init reads should be untracked.
    ui.set_unrelated(42.0);
    assert!(!window.draw_if_needed(|_| {
        unreachable!("changing a property only read by init must not trigger a redraw")
    }));
}

#[test]
fn issue_9882_borrow_mut() {
    slint::slint! {
        #[edition = "1.19"]
        export component App inherits Window {
            in-out property <[string]> strings: ["Home"];
            VerticalLayout {
                VerticalLayout {
                    for str in root.strings: Text {
                        text: str;
                    }
                }
            }
        }
    }

    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let ui = App::new().unwrap();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_strings(slint::ModelRc::new(slint::VecModel::from_iter([slint::SharedString::from(
        "Hello",
    )])));

    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));

    ui.set_strings(slint::ModelRc::new(slint::VecModel::from_iter([slint::SharedString::from(
        "World",
    )])));

    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
}

#[test]
/// Check that changing the position of a rectangle is properly tracked when RepaintBufferType::NewBuffer is used.
fn position_tracking_without_partial_rendering() {
    slint::slint! {
        #[edition = "1.19"]
        export component App inherits Window {
            width: 180px;
            height: 260px;

            in property <bool> cond: true;
            in property <length> rect-x: 10px;

            Rectangle {
                background: red;
                width: 40px;
                height: 80px;
                x: rect-x;
            }

            Rectangle {
                width: 100px;
                height: 100px;
                x: 1000px;
                background: cond ? green : blue;
            }
        }
    }

    struct TestPlatform2(Rc<MinimalSoftwareWindow>);
    impl slint::platform::Platform for TestPlatform2 {
        fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
            Ok(self.0.clone())
        }
    }
    let window = MinimalSoftwareWindow::new(
        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
    );
    slint::platform::set_platform(Box::new(TestPlatform2(window.clone()))).ok();
    let ui = App::new().unwrap();
    window.set_size(slint::PhysicalSize::new(180, 260));
    ui.show().unwrap();
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
    ui.set_cond(false);
    assert!(!window.draw_if_needed(|_| {
        unreachable!("the rectangle is outside the window so it should not be drawn")
    }));

    ui.set_rect_x(20.);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    ui.set_rect_x(-500.);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));

    ui.set_rect_x(20.);
    assert!(window.draw_if_needed(|renderer| {
        do_test_render_region(renderer, 0, 0, 180, 260);
    }));
    assert!(!window.draw_if_needed(|_| { unreachable!() }));
}

#[test]
fn destroy_never_rendered_item() {
    slint::slint! {
        #[edition = "1.19"]
        export component Ui inherits Window {
            in property <bool> c : false;
            background: black;
            Rectangle { x: 5px; y: 5px; width: 8px; height: 8px; background: green; }
            out property <int> instantiated;
            if c: Rectangle {
                x: 45px; y: 45px; width: 32px; height: 3px; background: pink;
                init => { root.instantiated += 1; }
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

    // Materialize the conditional between two frames without rendering it: the pointer
    // event's hit-testing instantiates the repeater's instance.
    ui.set_c(true);
    window.dispatch_event(slint::platform::WindowEvent::PointerMoved {
        position: slint::LogicalPosition::new(90., 90.),
    });
    assert_eq!(ui.get_instantiated(), 1);
    ui.set_c(false);

    // The instance is destroyed without ever having been rendered: nothing repaints.
    do_test_no_repaint(&window);
}
