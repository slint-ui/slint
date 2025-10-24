// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
mod wasm_backend;

slint::slint! {
    export { MainWindow } from "plotter.slint";
}

fn render_plot(pitch: f32, yaw: f32, amplitude: f32, width: u32, height: u32) -> slint::Image {
    fn probability_density_function(x: f64, y: f64, a: f64) -> f64 {
        const SDX: f64 = 0.1;
        const SDY: f64 = 0.1;
        let x = x / 10.0;
        let y = y / 10.0;
        a * (-x * x / 2.0 / SDX / SDX - y * y / 2.0 / SDY / SDY).exp()
    }

    use plotters::prelude::*;
    let mut pixel_buffer = slint::SharedPixelBuffer::new(width, height);
    {
        let size = (pixel_buffer.width(), pixel_buffer.height());
        let root =
            BitMapBackend::with_buffer(pixel_buffer.make_mut_bytes(), size).into_drawing_area();
        root.fill(&plotters::style::RGBColor(28, 28, 28)).expect("error filling drawing area");

        let mut chart = ChartBuilder::on(&root)
            .build_cartesian_3d(-3.0..3.0, 0.0..6.0, -3.0..3.0)
            .expect("error building coordinate system");
        chart.with_projection(|mut p| {
            p.pitch = pitch as f64;
            p.yaw = yaw as f64;
            p.scale = 0.62;
            p.into_matrix()
        });

        let gray = &plotters::style::RGBColor(64, 64, 64);
        chart
            .configure_axes()
            .label_style(("sans-serif", 19).into_font().color(&WHITE))
            .light_grid_style(gray)
            .bold_grid_style(gray)
            .max_light_lines(4)
            .draw()
            .expect("error drawing");
        let precision = 30;
        chart
            .draw_series(
                SurfaceSeries::xoz(
                    (-precision..=precision).map(|x| x as f64 / (precision as f64 / 3.0)),
                    (-precision..=precision).map(|x| x as f64 / (precision as f64 / 3.0)),
                    |x, y| probability_density_function(x, y, (amplitude as f64 / 1.0) * 6.0),
                )
                .style_func(&|&v| {
                    (&HSLColor(240.0 / 360.0 - 240.0 / 360.0 * v / 5.0, 1.0, 0.7)).into()
                }),
            )
            .expect("error drawing series");
        root.present().expect("error presenting");
    }

    slint::Image::from_rgb8(pixel_buffer)
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub fn main() {
    // This provides better error messages in debug mode.
    // It's disabled in release mode so it doesn't bloat up the file size.
    #[cfg(all(debug_assertions, target_arch = "wasm32"))]
    console_error_panic_hook::set_once();

    let main_window = MainWindow::new().expect("Cannot create main window");
    let main_window_weak = main_window.as_weak();

    let mut current_amplitude = -1.0f32;
    let mut current_pitch = -1.0f32;
    let mut current_yaw = -1.0f32;
    let mut current_width = 0u32;
    let mut current_height = 0u32;

    main_window
        .window()
        .set_rendering_notifier(move |state, _graphics_api| {
            if let slint::RenderingState::BeforeRendering = state {
                if let Some(main_window_strong) = main_window_weak.upgrade() {
                    let new_pitch = main_window_strong.get_pitch();
                    let new_yaw = main_window_strong.get_yaw();
                    let new_amplitude = main_window_strong.get_amplitude();
                    let new_width = main_window_strong.get_texture_width() as u32;
                    let new_height = main_window_strong.get_texture_height() as u32;
                    if current_pitch != new_pitch
                        || current_yaw != new_yaw
                        || current_amplitude != new_amplitude
                        || current_width != new_width
                        || current_height != new_height
                    {
                        current_pitch = new_pitch;
                        current_yaw = new_yaw;
                        current_amplitude = new_amplitude;
                        current_width = new_width;
                        current_height = new_height;
                        main_window_strong.set_texture(render_plot(
                            new_pitch,
                            new_yaw,
                            new_amplitude,
                            new_width,
                            new_height,
                        ));
                    }
                    main_window_strong.window().request_redraw();
                }
            }
        })
        .expect("Unable to set rendering notifier");

    main_window.run().expect("Failed to run main window");
}
