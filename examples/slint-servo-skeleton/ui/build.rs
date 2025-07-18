fn main() {
    // Compile app.slint and let Slint set the SLINT_INCLUDE_GENERATED var
    slint_build::compile("app.slint").unwrap();
}
