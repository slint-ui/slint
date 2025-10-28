// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

#[cfg(target_os = "linux")]
use gl_generator::{Api, Fallbacks, Profile, Registry, StructGenerator};

#[cfg(target_os = "linux")]
extern crate gl_generator;

fn main() {
    // Cargo does not expose the profile name to crates or their build scripts,
    // but we can extract it from OUT_DIR and set a custom cfg() ourselves.
    let out = env::var("OUT_DIR").unwrap();
    let out = Path::new(&out);

    #[cfg(target_os = "linux")]
    {
        let mut file = File::create(&out.join("gl_bindings.rs")).unwrap();

        // Config copied from https://github.com/YaLTeR/bxt-rs/blob/9f621251b8ce5c2af00b67d2feab731e48d1dae9/build.rs.

        Registry::new(
            Api::Gl,
            (4, 6),
            Profile::Compatibility,
            Fallbacks::All,
            [
                "GL_EXT_memory_object",
                "GL_EXT_memory_object_fd",
                "GL_EXT_memory_object_win32",
                "GL_EXT_semaphore",
                "GL_EXT_semaphore_fd",
                "GL_EXT_semaphore_win32",
            ],
        )
        .write_bindings(StructGenerator, &mut file)
        .unwrap();
    }

    // Note: We can't use `#[cfg(windows)]`, since that would check the host platform
    // and not the target platform
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();

    // On MacOS, all dylib dependencies are shipped along with the binary
    // in the "/lib" directory. Setting the rpath here, allows the dynamic
    // linker to locate them. See `man dyld` for more info.
    if target_os == "macos" {
        println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/lib/");
    }

    if target_os == "android" {
        // FIXME: We need this workaround since jemalloc-sys still links
        // to libgcc instead of libunwind, but Android NDK 23c and above
        // don't have libgcc. We can't disable jemalloc for Android as
        // in 64-bit aarch builds, the system allocator uses tagged
        // pointers by default which causes the assertions in SM & mozjs
        // to fail. See https://github.com/servo/servo/issues/32175.
        let mut libgcc = File::create(out.join("libgcc.a")).unwrap();
        libgcc.write_all(b"INPUT(-lunwind)").unwrap();
        println!("cargo:rustc-link-search=native={}", out.display());
    }

    println!("cargo:rerun-if-changed=/src/build.rs");

    slint_build::compile("ui/app.slint").unwrap();
}
