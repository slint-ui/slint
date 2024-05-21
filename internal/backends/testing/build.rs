// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    #[cfg(feature = "system-testing")]
    {
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let proto_file = std::path::PathBuf::from(::std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("slint_systest.proto");
        let config_builder = pb_rs::ConfigBuilder::new(&[proto_file], None, Some(&out_dir), &[])
            .unwrap()
            .headers(false)
            .dont_use_cow(true);
        pb_rs::types::FileDescriptor::run(&config_builder.build()).unwrap();
    }
}
