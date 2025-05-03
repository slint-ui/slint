// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::env;
use std::path::PathBuf;

use android_build::{Dexer, JavaBuild};

fn main() {
    if !env::var("TARGET").unwrap().contains("android") {
        return;
    }
    let release_mode = env::var("PROFILE").as_ref().map(|s| s.as_str()) == Ok("release");

    // This is the only Java source file
    let java_src = "SlintAndroidJavaHelper.java";
    let java_src_path = format!("java/{java_src}");

    let slint_path: PathBuf = ["dev", "slint", "android-activity"].iter().collect();

    let out_dir: PathBuf = env::var_os("OUT_DIR").unwrap().into();
    let mut out_class_dir = out_dir.clone();
    out_class_dir.push("java");
    out_class_dir.push(slint_path);

    if out_class_dir.try_exists().unwrap_or(false) {
        let _ = std::fs::remove_dir_all(&out_class_dir);
    }
    std::fs::create_dir_all(&out_class_dir)
        .unwrap_or_else(|e| panic!("Cannot create output directory {out_class_dir:?} - {e}"));

    let android_jar = android_build::android_jar(None).expect("No Android platforms found");

    // Compile the Java file into .class files
    let o = JavaBuild::new()
        .file(&java_src_path)
        .class_path(&android_jar)
        .classes_out_dir(&out_class_dir)
        .java_source_version(8)
        .java_target_version(8)
        .debug_info(android_build::DebugInfo {
            line_numbers: !release_mode,
            variables: !release_mode,
            source_files: !release_mode,
        })
        .command()
        .unwrap_or_else(|e| panic!("Could not generate the java compiler command: {e}"))
        .args(["-encoding", "UTF-8"])
        .output()
        .unwrap_or_else(|e| panic!("Could not run the java compiler: {e}"));

    if !o.status.success() {
        panic!("Java compilation failed: {}", String::from_utf8_lossy(&o.stderr));
    }

    let o = Dexer::new()
        .android_jar(&android_jar)
        .class_path(&out_class_dir)
        .collect_classes(&out_class_dir)
        .unwrap()
        .release(release_mode)
        .android_min_api(20) // disable multidex for single dex file output
        .out_dir(out_dir)
        .command()
        .unwrap_or_else(|e| panic!("Could not generate the D8 command: {e}"))
        .output()
        .unwrap_or_else(|e| panic!("Error running D8: {e}"));

    if !o.status.success() {
        eprintln!("Dex conversion failed: {}", String::from_utf8_lossy(&o.stderr));
        let javac = android_build::javac().unwrap();
        let java_ver = android_build::check_javac_version(&javac).unwrap();
        if java_ver >= 21 {
            eprintln!("WARNING: JDK version 21 is known to cause an error with older android SDK");
            eprintln!("See https://github.com/slint-ui/slint/issues/4973");
            eprintln!("Try downgrading your version of Java to something like JDK 17, or upgrade to the SDK build tools 35");
        }
        panic!("Dex conversion failed");
    }

    println!("cargo:rerun-if-changed={java_src_path}");
}
