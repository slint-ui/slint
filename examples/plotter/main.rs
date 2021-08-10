/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

use plotters::prelude::*;
use sixtyfps::SharedPixelBuffer;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

sixtyfps::sixtyfps! {
    import { MainWindow } from "plotter.60";
}

fn pdf(x: f64, y: f64) -> f64 {
    const SDX: f64 = 0.1;
    const SDY: f64 = 0.1;
    const A: f64 = 5.0;
    let x = x as f64 / 10.0;
    let y = y as f64 / 10.0;
    A * (-x * x / 2.0 / SDX / SDX - y * y / 2.0 / SDY / SDY).exp()
}

fn render_plot(pitch: f32) -> sixtyfps::Image {
    let mut pixel_buffer = SharedPixelBuffer::new(640, 480);
    let size = (pixel_buffer.width() as u32, pixel_buffer.height() as u32);

    let root = BitMapBackend::with_buffer(pixel_buffer.as_bytes_mut(), size).into_drawing_area();

    root.fill(&WHITE).expect("error filling drawing area");

    let mut chart = ChartBuilder::on(&root)
        .build_cartesian_3d(-3.0..3.0, 0.0..6.0, -3.0..3.0)
        .expect("error building coordinate system");
    chart.with_projection(|mut p| {
        p.pitch = 1.57 - (1.57 - pitch as f64 / 50.0).abs();
        p.scale = 0.7;
        p.into_matrix() // build the projection matrix
    });

    chart.configure_axes().draw().expect("error drawing");

    chart
        .draw_series(
            SurfaceSeries::xoz(
                (-15..=15).map(|x| x as f64 / 5.0),
                (-15..=15).map(|x| x as f64 / 5.0),
                pdf,
            )
            .style_func(&|&v| {
                (&HSLColor(240.0 / 360.0 - 240.0 / 360.0 * v / 5.0, 1.0, 0.7)).into()
            }),
        )
        .expect("error drawing series");

    root.present().expect("error presenting");
    drop(chart);
    drop(root);

    sixtyfps::Image::new_rgb8(pixel_buffer)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let main_window = MainWindow::new();

    main_window.on_redraw({
        let main_window_weak = main_window.as_weak();
        move || {
            let main_window = main_window_weak.upgrade().unwrap();
            let plot = render_plot(main_window.get_pitch());
            main_window.set_plot_image(plot);
        }
    });

    main_window.invoke_redraw();

    main_window.run();
}
