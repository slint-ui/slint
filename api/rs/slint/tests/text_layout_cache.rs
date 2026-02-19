// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use slint::PhysicalSize;
use slint::platform::software_renderer::{
    MinimalSoftwareWindow, PremultipliedRgbaColor, RepaintBufferType, SoftwareRenderer, TargetPixel,
};
use slint::platform::{PlatformError, WindowAdapter};
use std::rc::Rc;

thread_local! {
    static WINDOW: Rc<MinimalSoftwareWindow> =
        MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
}

struct TestPlatform;
impl slint::platform::Platform for TestPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(WINDOW.with(|x| x.clone()))
    }
}

#[allow(dead_code)]
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

const WIDTH: usize = 200;
const HEIGHT: usize = 100;

fn setup() -> Rc<MinimalSoftwareWindow> {
    slint::platform::set_platform(Box::new(TestPlatform)).ok();
    let window = WINDOW.with(|x| x.clone());
    window.set_size(PhysicalSize::new(WIDTH as u32, HEIGHT as u32));
    window
}

fn render_and_get_miss_count(renderer: &SoftwareRenderer) -> u64 {
    renderer.text_layout_cache().reset_cache_miss_count();
    let mut buf = vec![TestPixel(false); WIDTH * HEIGHT];
    renderer.render(buf.as_mut_slice(), WIDTH);
    renderer.text_layout_cache().cache_miss_count()
}

#[test]
fn cache_hit_avoids_reshaping() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            Text {
                text: "Hello World";
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    let mut miss_count = 0u64;

    // First render: should shape at least once
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected at least one cache miss on first render");

    // Second render without changes: should hit cache
    window.request_redraw();
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Expected zero cache misses on re-render without changes");
}

#[test]
fn text_change_invalidates_cache() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <string> label: "Hello";
            Text {
                text: label;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change text
    ui.set_label("Goodbye".into());

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected cache miss after text change");
}

#[test]
fn font_size_change_invalidates_cache() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <length> size: 16px;
            Text {
                text: "Hello";
                font-size: size;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change font-size
    ui.set_size(24.0);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected cache miss after font-size change");
}

#[test]
fn font_weight_change_invalidates_cache() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <int> weight: 400;
            Text {
                text: "Hello";
                font-weight: weight;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change font-weight
    ui.set_weight(700);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected cache miss after font-weight change");
}

#[test]
fn wrap_change_invalidates_cache() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <bool> use-no-wrap: false;
            Text {
                text: "Hello World this is a long text";
                wrap: use-no-wrap ? no-wrap : word-wrap;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render (word-wrap)
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change wrap to no-wrap
    ui.set_use_no_wrap(true);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert!(miss_count > 0, "Expected cache miss after wrap change");
}

#[test]
fn alignment_change_does_not_reshape() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <bool> use-center-align: false;
            Text {
                text: "Hello World";
                horizontal-alignment: use-center-align ? TextHorizontalAlignment.center : TextHorizontalAlignment.left;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render (left-aligned)
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change alignment to center
    ui.set_use_center_align(true);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Alignment change should not cause reshaping");
}

#[test]
fn overflow_change_does_not_reshape() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <bool> use-elide: false;
            Text {
                text: "Hello World";
                overflow: use-elide ? TextOverflow.elide : TextOverflow.clip;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render (clip)
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change overflow to elide
    ui.set_use_elide(true);

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Overflow change should not cause reshaping");
}

#[test]
fn color_change_does_not_reshape() {
    let window = setup();

    slint::slint! {
        export component TestComponent inherits Window {
            in property <color> text-color: black;
            Text {
                text: "Hello World";
                color: text-color;
            }
        }
    }

    let ui = TestComponent::new().unwrap();
    ui.show().unwrap();

    // First render
    window.draw_if_needed(|renderer| {
        render_and_get_miss_count(renderer);
    });

    // Change color
    ui.set_text_color(slint::Color::from_rgb_u8(255, 0, 0));

    let mut miss_count = 0u64;
    assert!(window.draw_if_needed(|renderer| {
        miss_count = render_and_get_miss_count(renderer);
    }));
    assert_eq!(miss_count, 0, "Color change should not cause reshaping");
}
