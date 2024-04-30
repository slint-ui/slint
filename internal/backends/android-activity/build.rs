// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.2 OR LicenseRef-Slint-commercial

use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

fn main() {
    if !env::var("TARGET").unwrap().contains("android") {
        return;
    }

    let out_dir: PathBuf = env::var_os("OUT_DIR").unwrap().into();

    let slint_path: PathBuf = ["dev", "slint", "android-activity"].iter().collect();
    let java_class = "SlintAndroidJavaHelper.java";

    let mut out_class = out_dir.clone();
    out_class.push("java");
    out_class.push(slint_path);

    let android_home =
        PathBuf::from(env_var("ANDROID_HOME").or_else(|_| env_var("ANDROID_SDK_ROOT")).expect(
            "Please set the ANDROID_HOME environment variable to the path of the Android SDK",
        ));

    let classpath = find_latest_version(android_home.join("platforms"), "android.jar")
        .expect("No Android platforms found");

    // Try to locate javac
    let javac_path = match env_var("JAVA_HOME") {
        Ok(val) => {
            if cfg!(windows) {
                format!("{}\\bin\\javac.exe", val)
            } else {
                format!("{}/bin/javac", val)
            }
        }
        Err(_) => String::from("javac"),
    };

    let handle_java_err = |err: std::io::Error| {
        if err.kind() == std::io::ErrorKind::NotFound {
            panic!("Could not locate the java compiler. Please ensure that the JAVA_HOME environment variable is set correctly.")
        } else {
            panic!("Could not run {javac_path}: {err}")
        }
    };

    // Check version
    let o = Command::new(&javac_path).arg("-version").output().unwrap_or_else(handle_java_err);
    if !o.status.success() {
        panic!("Failed to get javac version: {}", String::from_utf8_lossy(&o.stderr));
    }
    let mut version_output = String::from_utf8_lossy(&o.stdout);
    if version_output.is_empty() {
        // old version of java used stderr
        version_output = String::from_utf8_lossy(&o.stderr);
    }
    let version = version_output.split_whitespace().nth(1).unwrap_or_default();
    let mut java_ver: i32 = version.split('.').next().unwrap_or("0").parse().unwrap_or(0);
    if java_ver == 1 {
        // Before java 9, the version was something like javac 1.8
        java_ver = version.split('.').nth(1).unwrap_or("0").parse().unwrap_or(0);
    }
    if java_ver < 8 {
        panic!("The detected Java version is too old. The minimum required version is Java 8. Your Java version: {version_output:?} (parsed as {java_ver})")
    }

    // Compile the Java file into a .class file
    let o = Command::new(&javac_path)
        .arg(format!("java/{java_class}"))
        .arg("-d")
        .arg(out_class.as_os_str())
        .arg("-classpath")
        .arg(&classpath)
        .args(if java_ver != 8 { &["--release", "8"] } else { &[] as &[&str] })
        .args(&["-encoding", "UTF-8"])
        .output()
        .unwrap_or_else(handle_java_err);

    if !o.status.success() {
        panic!("Java compilation failed: {}", String::from_utf8_lossy(&o.stderr));
    }

    // Convert the .class file into a .dex file
    let d8_path = find_latest_version(
        android_home.join("build-tools"),
        if cfg!(windows) { "d8.bat" } else { "d8" },
    )
    .expect("d8 tool not found");

    // collect all the *.class files
    let classes = fs::read_dir(&out_class)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension() == Some(std::ffi::OsStr::new("class")))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();

    let o = Command::new(&d8_path)
        .arg("--classpath")
        .arg(&out_class)
        .args(&classes)
        .arg("--output")
        .arg(out_dir.as_os_str())
        .output()
        .unwrap_or_else(|err| panic!("Error running {d8_path:?}: {err}"));

    if !o.status.success() {
        panic!("Dex conversion failed: {}", String::from_utf8_lossy(&o.stderr));
    }

    println!("cargo:rerun-if-changed=java/{java_class}");
}

fn env_var(var: &str) -> Result<String, env::VarError> {
    println!("cargo:rerun-if-env-changed={}", var);
    env::var(var)
}

fn find_latest_version(base: PathBuf, arg: &str) -> Option<PathBuf> {
    fs::read_dir(base)
        .ok()?
        .filter_map(|entry| Some(entry.ok()?.path().join(arg)))
        .filter(|path| path.exists())
        .max()
}
