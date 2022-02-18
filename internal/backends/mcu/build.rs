// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

use std::path::Path;

fn main() -> std::io::Result<()> {
    let out_dir = std::env::var_os("OUT_DIR").unwrap();
    // out_dir is something like
    // <target_dir>/build/i-slint-backend-mcu-1fe5c4ab61eb0584/out
    // and we want to write to a common directory, so write in the build/ dir
    let target_path = Path::new(&out_dir)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("SLINT_MCU_BOARD_CONFIG_PATH.txt");

    #[allow(unused)]
    let mut config: Option<std::path::PathBuf> = None;

    cfg_if::cfg_if! {
        if #[cfg(feature = "pico-st7789")] {
            config = Some([env!("CARGO_MANIFEST_DIR"), "pico_st7789", "board_config.toml"].iter().collect());
        }
    }
    std::fs::write(
        target_path,
        config
            .map_or(std::borrow::Cow::Borrowed(b"" as &[u8]), |path| {
                path.to_string_lossy().as_bytes().to_vec().into()
            })
            .as_ref(),
    )
}
