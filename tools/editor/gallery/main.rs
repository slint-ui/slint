// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

mod catalog;
mod controller;

use clap::Parser;
use slint::ComponentHandle;
use slint_editor::ui::GalleryWindow;

#[derive(Parser)]
#[command(about = "Interactive gallery of Visual Editor components")]
struct Args {
    #[arg(long, default_value = "palette")]
    page: String,
    #[arg(long, default_value = "Default")]
    scenario: String,
    #[arg(long, value_parser = ["system", "light", "dark"], default_value = "system")]
    theme: String,
    #[arg(long)]
    list: bool,
    #[arg(long, default_value_t = 1440, value_parser = clap::value_parser!(u32).range(800..=3840))]
    width: u32,
    #[arg(long, default_value_t = 1000, value_parser = clap::value_parser!(u32).range(600..=2160))]
    height: u32,
    #[arg(long, value_parser = parse_scale)]
    scale_factor: Option<f32>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.list {
        for page in catalog::PAGES {
            println!("{}: {}", page.id, page.scenarios.join(", "));
        }
        return Ok(());
    }
    let page = catalog::page(&args.page)
        .ok_or_else(|| format!("Unknown page: {}. Use --list.", args.page))?;
    if !page.scenarios.contains(&args.scenario.as_str()) {
        return Err(
            format!("Unknown scenario for {}: {}. Use --list.", args.page, args.scenario).into()
        );
    }
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
    window.set_theme_index(match args.theme.as_str() {
        "light" => 1,
        "dark" => 2,
        _ => 0,
    });
    controller::install(&window);
    controller::navigate(&window, &args.page, &args.scenario);
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
