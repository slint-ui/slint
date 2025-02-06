// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::path::PathBuf;
use std::process::Command;
use std::{env, fs};

fn main() {
    // unit test: `#[test]` fn are not run in build.rs, so we need to call it manually
    test_parse_version();

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

    // Try to locate javac and java
    let javac_java = env_var("JAVA_HOME")
        .map(|home| PathBuf::from(home).join("bin"))
        .map(|bin| (bin.join("javac"), bin.join("java")));
    let (javac_path, java_path) = if let Ok(ref javac_java) = javac_java {
        (javac_java.0.to_str().unwrap(), javac_java.1.to_str().unwrap())
    } else {
        ("javac", "java")
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
    let java_ver = parse_javac_version_output(&version_output);
    if java_ver < 8 {
        panic!("The detected Java version is too old. The minimum required version is Java 8. Your Java version: {version_output:?} (parsed as {java_ver})")
    }

    std::fs::create_dir_all(&out_class)
        .unwrap_or_else(|e| panic!("Cannot create output directory {out_class:?} - {e}"));

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
    let d8_path = find_latest_version(android_home.join("build-tools"), "lib")
        .map(|path| path.join("d8.jar"))
        .expect("d8 tool not found");

    // collect all the *.class files
    let classes = fs::read_dir(&out_class)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension() == Some(std::ffi::OsStr::new("class")))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();

    let o = Command::new(&java_path)
        // class path of D8 itself
        .arg("-classpath")
        .arg(&d8_path)
        .arg("com.android.tools.r8.D8")
        // class path of D8's input
        .arg("--classpath")
        .arg(&out_class)
        .args(&classes)
        .arg("--output")
        .arg(out_dir.as_os_str())
        // workaround for the DexClassLoader in Android 7.x
        .arg("--min-api")
        .arg("20")
        .output()
        .unwrap_or_else(|err| panic!("Error running {d8_path:?}: {err}"));

    if !o.status.success() {
        eprintln!("Dex conversion failed: {}", String::from_utf8_lossy(&o.stderr));

        if java_ver >= 21 {
            eprintln!("WARNING: JDK version 21 is known to cause an error with older android SDK");
            eprintln!("See https://github.com/slint-ui/slint/issues/4973");
            eprintln!("Try downgrading your version of Java to something like JDK 17, or upgrade to the SDK build tools 35");
        }
        panic!("Dex conversion failed");
    }

    println!("cargo:rerun-if-changed=java/{java_class}");
}

fn parse_javac_version_output(version_output: &str) -> i32 {
    let version = version_output
        .split_whitespace()
        .nth(1)
        .and_then(|v| v.split('-').next())
        .unwrap_or_default();
    let mut java_ver: i32 = version.split('.').next().unwrap_or("0").parse().unwrap_or(0);
    if java_ver == 1 {
        // Before java 9, the version was something like javac 1.8
        java_ver = version.split('.').nth(1).unwrap_or("0").parse().unwrap_or(0);
    }
    java_ver
}

//#[test] doesn't work in build.rs so it is called from main. Impact on build time should be negligible
fn test_parse_version() {
    assert_eq!(parse_javac_version_output("javac 1.8.0_292"), 8);
    assert_eq!(parse_javac_version_output("javac 17.0.13"), 17);
    assert_eq!(parse_javac_version_output("javac 21.0.5"), 21);
    assert_eq!(parse_javac_version_output("javac 24-ea"), 24);
    assert_eq!(parse_javac_version_output("error"), 0);
    assert_eq!(parse_javac_version_output("javac error"), 0);
}

fn env_var(var: &str) -> Result<String, env::VarError> {
    println!("cargo:rerun-if-env-changed={var}");
    env::var(var)
}

fn find_latest_version(base: PathBuf, arg: &str) -> Option<PathBuf> {
    fs::read_dir(base)
        .ok()?
        .filter_map(|entry| Some(entry.ok()?.path().join(arg)))
        .filter(|path| path.exists())
        .max()
}
