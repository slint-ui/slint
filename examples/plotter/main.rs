// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use plotters::prelude::*;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
mod wasm_backend;

slint::slint! {
    export { MainWindow } from "plotter.slint";
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct PlotVariables {
    pitch: f32,
    yaw: f32,
    amplitude: f32,
    width: u32,
    height: u32,
}

fn render_plot(vars: &PlotVariables) -> slint::Image {
    fn probability_density_function(x: f64, y: f64, a: f64) -> f64 {
        const SDX: f64 = 0.1;
        const SDY: f64 = 0.1;
        let x = x / 10.0;
        let y = y / 10.0;
        a * (-x * x / 2.0 / SDX / SDX - y * y / 2.0 / SDY / SDY).exp()
    }

    let mut pixel_buffer = slint::SharedPixelBuffer::new(vars.width, vars.height);
    {
        let size = (pixel_buffer.width(), pixel_buffer.height());
        let backend = BitMapBackend::with_buffer(pixel_buffer.make_mut_bytes(), size);

        // Plotters requires TrueType fonts from the file system to draw axis text - we skip that for
        // WASM for now.
        #[cfg(target_arch = "wasm32")]
        let backend = wasm_backend::BackendWithoutText { backend };

        let root = backend.into_drawing_area();
        root.fill(&WHITE).expect("error filling drawing area");

        let mut chart = ChartBuilder::on(&root)
            .build_cartesian_3d(-3.0..3.0, 0.0..6.0, -3.0..3.0)
            .expect("error building chart");
        chart.with_projection(|mut p| {
            p.pitch = vars.pitch as f64;
            p.yaw = vars.yaw as f64;
            p.scale = 0.62;
            p.into_matrix()
        });
        chart.configure_axes().draw().expect("error drawing");
        chart
            .draw_series(
                SurfaceSeries::xoz(
                    (-30..=30).map(|x| x as f64 / 10.0),
                    (-30..=30).map(|x| x as f64 / 10.0),
                    |x, y| probability_density_function(x, y, (vars.amplitude as f64 / 1.0) * 6.0),
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

    let mut plot_variables =
        PlotVariables { pitch: -1.0, yaw: -1.0, amplitude: -1.0, width: 0, height: 0 };

    main_window
        .window()
        .set_rendering_notifier(move |state, _| {
            if let slint::RenderingState::BeforeRendering = state {
                if let Some(main_window) = main_window_weak.upgrade() {
                    let current_plot_variables = PlotVariables {
                        pitch: main_window.get_pitch(),
                        yaw: main_window.get_yaw(),
                        amplitude: main_window.get_amplitude(),
                        width: main_window.get_texture_width() as u32,
                        height: main_window.get_texture_height() as u32,
                    };

                    if current_plot_variables != plot_variables {
                        plot_variables = current_plot_variables;
                        main_window.set_texture(render_plot(&current_plot_variables));
                    }
                    main_window.window().request_redraw();
                }
            }
        })
        .expect("Unable to set rendering notifier");

    main_window.run().expect("Failed to run main window");
}
