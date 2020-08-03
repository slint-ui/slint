#![deny(unsafe_code)]

sixtyfps::include_modules!();

fn main() {
    let app = Hello::new();
    let app_weak = sixtyfps::re_exports::PinWeak::downgrade(app.clone());
    app.as_ref().on_plus_clicked(move || {
        let app = app_weak.upgrade().unwrap();
        app.as_ref().set_counter(app.as_ref().get_counter() + 1);
    });
    let app_weak = sixtyfps::re_exports::PinWeak::downgrade(app.clone());
    app.as_ref().on_minus_clicked(move || {
        let app = app_weak.upgrade().unwrap();
        app.as_ref().set_counter(app.as_ref().get_counter() - 1);
    });
    app.run();
}
