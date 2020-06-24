#![deny(unsafe_code)]

sixtyfps::include_modules!();

fn main() {
    let mut app = Hello::default();

    app.plus_clicked.set_handler(|context, ()| {
        let app = context.get_component::<Hello>().unwrap();
        app.counter.set(app.counter.get(context) + 1);
    });
    app.minus_clicked.set_handler(|context, ()| {
        let app = context.get_component::<Hello>().unwrap();
        app.counter.set(app.counter.get(context) - 1);
    });
    app.run();
}
