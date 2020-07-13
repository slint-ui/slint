#![deny(unsafe_code)]

sixtyfps::include_modules!();

fn main() {
    let app = Hello::new();
    let app_weak = sixtyfps::re_exports::WeakPin::downgrade(app.clone());
    app.plus_clicked.set_handler(move |()| {
        let app = app_weak.upgrade().unwrap();
        let counter = Hello::field_offsets().counter.apply_pin(app.as_ref());
        counter.set(counter.get() + 1);
    });
    let app_weak = sixtyfps::re_exports::WeakPin::downgrade(app.clone());
    app.minus_clicked.set_handler(move |()| {
        let app = app_weak.upgrade().unwrap();
        let counter = Hello::field_offsets().counter.apply_pin(app.as_ref());
        counter.set(counter.get() - 1);
    });
    app.run();
}
