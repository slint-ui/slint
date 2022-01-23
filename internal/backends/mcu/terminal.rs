// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::io::Write;

use crate::lengths::*;
use crossterm::cursor::MoveTo;
use crossterm::event::{MouseButton, MouseEventKind};
use crossterm::style::{style, Stylize};
use crossterm::{ExecutableCommand, QueueableCommand};
use embedded_graphics::pixelcolor::RgbColor;
use i_slint_core::input::MouseEvent;
use i_slint_core::items::PointerEventButton;

use crate::Devices;

// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

pub fn init() {
    std::io::stdout().execute(crossterm::terminal::EnterAlternateScreen).unwrap();
    std::io::stdout().execute(crossterm::event::EnableMouseCapture).unwrap();
    crossterm::terminal::enable_raw_mode().unwrap();
    crate::init_with_display(TerminalDevices {
        frame_buffer: Default::default(),
        density: std::env::var("SLINT_TERMINAL_DENSITY")
            .ok()
            .and_then(|x| x.parse().ok())
            .unwrap_or(2),
    });
}

struct TerminalDevices {
    frame_buffer: Vec<super::TargetPixel>,
    /// the amount of pixel per character
    density: u8,
}

impl Devices for TerminalDevices {
    fn screen_size(&self) -> PhysicalSize {
        let (w, h) = crossterm::terminal::size().unwrap();
        euclid::size2(w, h * self.density as u16).cast()
    }

    fn fill_region(&mut self, region: PhysicalRect, pixels: &[super::TargetPixel]) {
        let mut stdout = std::io::stdout();
        let w = region.width();

        if self.density > 1 {
            let a_size = self.screen_size().to_usize();
            if a_size.width * a_size.height > self.frame_buffer.len() {
                self.frame_buffer.resize(a_size.width * a_size.height, super::TargetPixel::BLACK);
            }

            for (it, pix) in pixels.iter().copied().enumerate() {
                let idx = region.min_x() as usize
                    + it % w as usize
                    + (region.min_y() as usize + it / w as usize) * a_size.width;
                self.frame_buffer[idx] = pix;
            }

            for y in (0..region.height()).step_by(2) {
                for x in 0..w {
                    let a_x = region.min_x() + x;
                    let a_y = (region.min_y() + y) / 2;
                    let p1 = (a_y * 2 - region.min_y() >= 0)
                        .then(|| ((a_y * 2 - region.min_y()) * w + x) as usize)
                        .and_then(|idx| pixels.get(idx))
                        .or_else(|| {
                            self.frame_buffer.get(a_x as usize + 2 * a_y as usize * a_size.width)
                        })
                        .copied()
                        .map_or(crossterm::style::Color::Reset, map_color);
                    let p2 = pixels
                        .get(((a_y * 2 - region.min_y() + 1) * w + x) as usize)
                        .or_else(|| {
                            self.frame_buffer
                                .get(a_x as usize + (1 + 2 * a_y as usize) * a_size.width)
                        })
                        .copied()
                        .map_or(crossterm::style::Color::Reset, map_color);
                    stdout
                        .queue(MoveTo(a_x as u16, a_y as u16))
                        .unwrap()
                        .queue(crossterm::style::PrintStyledContent(style("▀").with(p1).on(p2)))
                        .unwrap();
                }
            }
        } else {
            for (it, pix) in pixels.iter().copied().enumerate() {
                let x = (region.min_x() + it as i16 % w) as u16;
                let y = (region.min_y() + it as i16 / w) as u16;
                stdout
                    .queue(MoveTo(x, y))
                    .unwrap()
                    .queue(crossterm::style::PrintStyledContent(style("█").with(map_color(pix))))
                    .unwrap();
            }
        }
        stdout.flush().unwrap();
    }

    fn debug(&mut self, _: &str) {}

    fn read_touch_event(&mut self) -> Option<MouseEvent> {
        // wait some millisecond to throttle the event loop
        if crossterm::event::poll(core::time::Duration::from_millis(5)).unwrap() {
            match crossterm::event::read().unwrap() {
                crossterm::event::Event::Mouse(m) => {
                    let pos = euclid::point2(m.column, m.row * 2 + 1).cast();
                    Some(match m.kind {
                        MouseEventKind::Down(btn) => {
                            MouseEvent::MousePressed { pos, button: map_button(btn) }
                        }
                        MouseEventKind::Up(btn) => {
                            MouseEvent::MouseReleased { pos, button: map_button(btn) }
                        }
                        MouseEventKind::Drag(_) | MouseEventKind::Moved => {
                            MouseEvent::MouseMoved { pos }
                        }
                        MouseEventKind::ScrollDown => {
                            MouseEvent::MouseWheel { pos, delta: euclid::point2(0., 1.) }
                        }
                        MouseEventKind::ScrollUp => {
                            MouseEvent::MouseWheel { pos, delta: euclid::point2(0., -1.) }
                        }
                    })
                }
                crossterm::event::Event::Key(_) => {
                    // FIXME
                    quit();
                }
                _ => None,
            }
        } else {
            None
        }
    }

    fn time(&self) -> core::time::Duration {
        static INITIAL_INSTANT: once_cell::race::OnceBox<std::time::Instant> =
            once_cell::race::OnceBox::new();
        let the_beginning = *INITIAL_INSTANT.get_or_init(|| Box::new(std::time::Instant::now()));
        std::time::Instant::now() - the_beginning
    }
}

fn map_color(pix: super::TargetPixel) -> crossterm::style::Color {
    let pix = embedded_graphics::pixelcolor::Rgb888::from(pix);
    crossterm::style::Color::Rgb { r: pix.r(), g: pix.g(), b: pix.b() }
}

fn map_button(btn: MouseButton) -> PointerEventButton {
    match btn {
        MouseButton::Left => PointerEventButton::left,
        MouseButton::Right => PointerEventButton::right,
        MouseButton::Middle => PointerEventButton::middle,
    }
}

fn quit() -> ! {
    crossterm::terminal::disable_raw_mode().unwrap();
    std::io::stdout().execute(crossterm::terminal::LeaveAlternateScreen).unwrap();
    std::io::stdout().execute(crossterm::event::DisableMouseCapture).unwrap();
    std::process::exit(0);
}
