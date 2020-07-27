use std::process::Command;

fn qmake_query(var: &str) -> Option<String> {
    let qmake = std::env::var_os("QMAKE").unwrap_or("qmake".into());
    Command::new(qmake).env("QT_SELECT", "qt5").args(&["-query", var]).output().ok().map(|output| {
        String::from_utf8(output.stdout).expect("UTF-8 conversion from ouytput of qmake failed")
    })
}

fn main() {
    if qmake_query("QT_VERSION").is_some() {
        println!("cargo:rustc-cfg=have_qt");
    }
}
