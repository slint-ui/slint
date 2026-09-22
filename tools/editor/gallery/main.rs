// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use clap::Parser;
use i_slint_core::DataTransfer;
use slint::{ComponentHandle, SharedString};
use slint_editor::{
    component_support::{brushes, element_library, recent_fills},
    ui::{Api, Gallery, GalleryWindow},
};

#[derive(Parser)]
#[command(about = "Interactive gallery of Visual Editor components")]
struct Args {
    #[arg(long, default_value_t = 1440, value_parser = clap::value_parser!(u32).range(800..=3840))]
    width: u32,
    #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u32).range(600..=2160))]
    height: u32,
    #[arg(long, value_parser = parse_scale)]
    scale_factor: Option<f32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let window = GalleryWindow::new()?;
    let size = slint::LogicalSize::new(args.width as f32, args.height as f32);
    if let Some(scale_factor) = args.scale_factor {
        window.window().set_size(slint::PhysicalSize::new(
            (size.width * scale_factor).round() as u32,
            (size.height * scale_factor).round() as u32,
        ));
        window
            .window()
            .dispatch_event(slint::platform::WindowEvent::ScaleFactorChanged { scale_factor });
        window.window().dispatch_event(slint::platform::WindowEvent::Resized { size });
    } else {
        window.window().set_size(size);
    }
    let api = window.global::<Api>();
    brushes::setup(&api);
    element_library::setup(&api);
    recent_fills::setup(&api, <Api as slint::Global<'_, GalleryWindow>>::as_weak(&api));
    window.global::<Gallery>().on_matches(|text, query| {
        text.to_lowercase().contains(query.trim().to_lowercase().as_str())
    });
    api.on_new_component_data_for_kind(|kind| {
        DataTransfer::from(SharedString::from(format!("{kind:?}")))
    });
    api.on_move_element_instance_data(|_, id| {
        DataTransfer::from(SharedString::from(format!("Row {id}")))
    });
    let weak = window.as_weak();
    api.on_drop(move |data, _, _| {
        if let Some(window) = weak.upgrade() {
            window
                .global::<Gallery>()
                .set_feedback(format!("Dropped {}", data.plain_text().unwrap_or_default()).into());
        }
    });
    window.global::<Gallery>().invoke_navigate(window.global::<Gallery>().get_page_index(), 0);
    window.run()?;
    Ok(())
}

fn parse_scale(value: &str) -> Result<f32, String> {
    value
        .parse::<f32>()
        .ok()
        .filter(|v| (0.5..=4.0).contains(v))
        .ok_or_else(|| "Scale factor must be between 0.5 and 4.".into())
}
