// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use slint::platform::software_renderer::{MinimalSoftwareWindow, Rgb565Pixel, TargetPixel};
use slint::platform::{PlatformError, WindowAdapter};
use std::rc::Rc;

slint::include_modules!();

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

thread_local!(
    static WINDOW: Rc<MinimalSoftwareWindow> = MinimalSoftwareWindow::new(
        slint::platform::software_renderer::RepaintBufferType::NewBuffer,
    );
);

struct BenchPlatform;
impl slint::platform::Platform for BenchPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(WINDOW.with(|x| x.clone()))
    }
}

const SIZE: slint::PhysicalSize = slint::PhysicalSize { width: 480, height: 320 };

#[derive(Debug, Copy, Clone, PartialEq)]
enum RenderMode {
    LineByLine,
    FullBuffer,
}

struct DrawBuffer<'a, T>(&'a mut [T]);

impl<T: TargetPixel> slint::platform::software_renderer::LineBufferProvider for DrawBuffer<'_, T> {
    type TargetPixel = T;
    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        render_fn(&mut self.0[line * SIZE.width as usize..][range]);
    }
}

fn do_rendering(
    window: &MinimalSoftwareWindow,
    buffer: &mut [impl TargetPixel],
    mode: RenderMode,
) -> bool {
    window.request_redraw();
    window.draw_if_needed(|renderer| {
        match mode {
            RenderMode::LineByLine => renderer.render_by_line(DrawBuffer(buffer)),
            RenderMode::FullBuffer => renderer.render(buffer, SIZE.width as usize),
        };
    })
}

#[divan::bench(
    types = [Rgb565Pixel, slint::Rgb8Pixel],
    args = [RenderMode::LineByLine, RenderMode::FullBuffer]
)]
fn render_only<T: TargetPixel + Default>(bencher: divan::Bencher, mode: RenderMode) {
    let _ = slint::platform::set_platform(Box::new(BenchPlatform));
    let main_window = MainWindow::new().unwrap();
    let _ = main_window.show();
    main_window.window().set_size(SIZE);
    let mut buffer = vec![T::default(); (SIZE.width * SIZE.height) as usize];

    WINDOW.with(|window| {
        // Do a first rendering to evaluate bindings
        let ok = do_rendering(window, &mut buffer, mode);
        assert!(ok);

        bencher.bench_local(|| {
            let ok = do_rendering(window, &mut buffer, mode);
            assert!(ok);
        })
    });
}

#[divan::bench(types = [Rgb565Pixel, slint::Rgb8Pixel])]
fn full<T: TargetPixel + Default>(bencher: divan::Bencher) {
    let _ = slint::platform::set_platform(Box::new(BenchPlatform));
    let mut buffer = vec![T::default(); (480 * 320) as usize];
    WINDOW.with(|window| {
        bencher.bench_local(|| {
            let main_window = MainWindow::new().unwrap();
            let _ = main_window.show();
            main_window.window().set_size(SIZE);
            let ok = do_rendering(window, &mut buffer, RenderMode::FullBuffer);
            assert!(ok);
        })
    });
}

fn main() {
    divan::main();
}
