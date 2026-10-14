// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Compile with `rustc --edition=2024 --test host.rs -o /tmp/frame-queue-tests`.
//! Host substitutes cover packet packing without linking the ESP32 HAL.

extern crate alloc;
extern crate self as i_slint_core;
extern crate self as slint;

#[macro_export]
macro_rules! thread_local {
    ($($tokens:tt)*) => { std::thread_local!($($tokens)*); };
}

#[path = "../animation_frame.rs"]
mod animation_frame;

pub mod platform {
    pub mod software_renderer {
        #[derive(Clone, Copy)]
        #[repr(transparent)]
        pub struct Rgb565BigEndianPixel(pub u16);

        #[derive(Clone, Copy)]
        pub struct Position {
            pub x: i32,
            pub y: i32,
        }

        #[derive(Clone, Copy)]
        pub struct Size {
            pub width: u32,
            pub height: u32,
        }

        #[derive(Default)]
        pub struct PhysicalRegion(pub Vec<(Position, Size)>);

        impl PhysicalRegion {
            pub fn iter(&self) -> impl Iterator<Item = (Position, Size)> + '_ {
                self.0.iter().copied()
            }

            pub fn bounding_box_origin(&self) -> Position {
                Position {
                    x: self.0.iter().map(|(p, _)| p.x).min().unwrap_or(0),
                    y: self.0.iter().map(|(p, _)| p.y).min().unwrap_or(0),
                }
            }

            pub fn bounding_box_size(&self) -> Size {
                let origin = self.bounding_box_origin();
                Size {
                    width: self
                        .0
                        .iter()
                        .map(|(p, s)| p.x + s.width as i32)
                        .max()
                        .unwrap_or(0)
                        .saturating_sub(origin.x) as u32,
                    height: self
                        .0
                        .iter()
                        .map(|(p, s)| p.y + s.height as i32)
                        .max()
                        .unwrap_or(0)
                        .saturating_sub(origin.y) as u32,
                }
            }
        }
    }
}

#[cfg(feature = "box-profile")]
mod profile {
    pub fn mode() -> u32 {
        0
    }
}

fn now_us() -> u64 {
    42
}

#[path = "../frame_queue.rs"]
mod frame_queue;
