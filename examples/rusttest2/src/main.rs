#![deny(unsafe_code)]

sixtyfps::include_modules!();

fn main() {
    let app = Hello::new();

    app.plus_clicked.set_handler(|context, ()| {
        let app = context.get_component::<Hello>().unwrap();
        let counter = Hello::field_offsets().counter.apply_pin(app);
        counter.set(counter.get(context) + 1);
    });
    app.minus_clicked.set_handler(|context, ()| {
        let app = context.get_component::<Hello>().unwrap();
        let counter = Hello::field_offsets().counter.apply_pin(app);
        counter.set(counter.get(context) - 1);
    });
    app.run();
}
