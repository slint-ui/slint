// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

fn main() {
    #[cfg(any(feature = "system-testing", feature = "mcp"))]
    {
        use prost::Message;
        let manifest_dir =
            std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let proto_file = manifest_dir.join("slint_systest.proto");
        let fds = protox::compile([&proto_file], [&manifest_dir])
            .expect("failed to compile slint_systest.proto");
        let descriptor_bytes = fds.encode_to_vec();

        prost_build::Config::new()
            .compile_fds(fds)
            .expect("failed to generate Rust code from proto descriptors");

        pbjson_build::Builder::new()
            .register_descriptors(&descriptor_bytes)
            .expect("failed to register proto descriptors for pbjson")
            .out_dir(&out_dir)
            .build(&[".proto"])
            .expect("failed to generate serde impls from proto descriptors");
    }
}
