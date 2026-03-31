// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    #[cfg(feature = "system-testing")]
    {
        let manifest_dir =
            std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let proto_file = manifest_dir.join("slint_systest.proto");
        let fds = protox::compile([&proto_file], [&manifest_dir])
            .expect("failed to compile slint_systest.proto");
        prost_build::Config::new()
            .compile_fds(fds)
            .expect("failed to generate Rust code from proto descriptors");
    }
}
