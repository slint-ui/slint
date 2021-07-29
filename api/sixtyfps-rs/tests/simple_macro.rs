use ::sixtyfps::sixtyfps;

#[test]
fn simple_window() {
    sixtyfps!(X := Window{});
    X::new();
}
#[test]
fn empty_stuff() {
    sixtyfps!();
    sixtyfps!(struct Hei := { abcd: bool });
}
