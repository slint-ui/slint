
mod resource_loader;

use resource_loader::{load_slint_image, set_resource_provider, FileSystemProvider};
use std::path::PathBuf;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Step 1: Register the resource provider BEFORE creating any UI.
    // In a real app this could be a custom provider that loads from
    // Qt resources, a compressed archive, a database, etc.
    let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets");
    set_resource_provider(Box::new(FileSystemProvider::new(assets_dir)));

    println!("[main] Resource provider registered.");

    // Step 2: Create the Slint window.
    let window = AppWindow::new()?;

    // Step 3: Wire up the load-image callback.
    // When the user clicks the button, this closure runs.
    // It calls the provider, decodes the bytes, and sets the image property.
    let window_weak = window.as_weak();

    window.on_load_image(move || {
        let window = window_weak.unwrap();

        // Path is relative — just like in a .slint file.
        // The provider decides where to actually find it.
        match load_slint_image("images/demo.png") {
            Ok(img) => {
                window.set_loaded_image(img);
                window.set_status_text(
                    "✓ Image loaded at runtime from assets/images/demo.png".into(),
                );
                println!("[main] Image loaded successfully.");
            }
            Err(e) => {
                window.set_status_text(format!("✗ Could not load image: {e}").into());
                eprintln!("[main] Failed to load image: {e}");
            }
        }
    });

    // Step 4: Run the event loop.
    window.run()?;

    Ok(())
}