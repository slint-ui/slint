sixtyfps::sixtyfps! {
    MainWindow := Window {
        Text {
            text: "hello world";
            color: green;
        }
    }
}
fn main() {
    MainWindow::new().run();
}
