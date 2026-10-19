// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::{
    ComponentHandle,
    platform::{
        Platform, WindowAdapter,
        software_renderer::{MinimalSoftwareWindow, RepaintBufferType, Rgb565BigEndianPixel},
    },
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
#[path = "../../../native_buffer.rs"]
mod native_buffer;

slint::slint! {
    export component Fixture inherits Window {
        width: 320px;
        height: 240px;
        background: #2a2846;
        in property <image> tile;
        in property <length> dx;
        in property <length> dy;
        in property <float> fade: 1;
        in property <length> tile-width: 48px;
        Image { x: root.dx; y: root.dy; width: root.tile-width; height: 140px; source: root.tile; opacity: root.fade; image-rendering: pixelated; image-fit: fill; }
        Rectangle {
            x: 110px; y: 70px; width: 31px; height: 79px; clip: true;
            Image { x: -7px; y: -9px; width: 48px; height: 140px; source: root.tile; }
        }
        Rectangle { x: 47px; y: 80px; width: 32px; height: 31px; background: #fa8a; }
    }
}

struct Backend {
    time: Rc<Cell<u64>>,
    windows: Rc<RefCell<Vec<Rc<MinimalSoftwareWindow>>>>,
}
impl Platform for Backend {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(if self.windows.borrow().is_empty() {
            RepaintBufferType::ReusedBuffer
        } else {
            RepaintBufferType::NewBuffer
        });
        self.windows.borrow_mut().push(window.clone());
        Ok(window)
    }
    fn duration_since_start(&self) -> Duration {
        Duration::from_millis(self.time.get())
    }
}

fn main() {
    let time = Rc::new(Cell::new(0));
    let windows = Rc::new(RefCell::new(Vec::new()));
    slint::platform::set_platform(Box::new(Backend {
        time: time.clone(),
        windows: windows.clone(),
    }))
    .unwrap();
    let accelerated = Fixture::new().unwrap();
    let canonical = Fixture::new().unwrap();
    for ui in [&accelerated, &canonical] {
        ui.show().unwrap();
    }
    let mut actual = vec![Rgb565BigEndianPixel(0); 320 * 240];
    let mut expected = actual.clone();
    for tick in 0..720 {
        let mut tile = slint::SharedPixelBuffer::<slint::Rgb565Pixel>::new(48, 140);
        for (index, pixel) in tile.make_mut_slice().iter_mut().enumerate() {
            pixel.0 = (index as u16).wrapping_mul(137).wrapping_add(tick as u16 * 41);
        }
        let image = slint::Image::from_rgb565(tile);
        for ui in [&accelerated, &canonical] {
            ui.set_tile(image.clone());
            ui.set_dx((tick % 380) as f32 - 50.);
            ui.set_dy((tick % 280) as f32 - 30.);
            ui.set_fade(if tick % 7 == 0 { 0.45 } else { 1. });
            ui.set_tile_width(if tick % 5 == 0 { 54. } else { 48. });
        }
        time.set(tick * 17);
        slint::platform::update_timers_and_animations();
        windows.borrow()[0].draw_if_needed(|r| {
            r.render_into_buffer(&mut native_buffer::FrameBuffer::new(&mut actual, 320));
        });
        windows.borrow()[1].draw_if_needed(|r| {
            r.render(&mut expected, 320);
        });
        assert_eq!(actual, expected, "texture parity at frame {tick}");
    }
    println!(
        "720 retained frames match canonical rendering, including clipping, alpha and scaled fallback"
    );
}
