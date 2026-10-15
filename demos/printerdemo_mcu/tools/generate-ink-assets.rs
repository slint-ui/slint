// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

extern crate alloc;
#[allow(dead_code)]
mod renderer {
    include!("../ink_particles.rs");

    pub fn write_assets(destination: &std::path::Path) {
        std::fs::create_dir_all(destination).unwrap();
        for index in 0..4 {
            let vial = Vial::build(48, 140, index, false);
            for (name, pixels) in [("empty", &vial.empty), ("liquid", &vial.liquid)] {
                let bytes: Vec<u8> = pixels.iter().flat_map(|p| p.to_le_bytes()).collect();
                std::fs::write(destination.join(format!("vial-{index}-{name}.rgb565")), bytes)
                    .unwrap();
            }
            std::fs::write(destination.join(format!("vial-{index}-mask.alpha")), &vial.mask)
                .unwrap();
        }
    }
}
fn main() {
    let destination = std::env::args_os().nth(1).expect("pass the asset output directory");
    renderer::write_assets(std::path::Path::new(&destination));
}
