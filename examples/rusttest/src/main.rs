// Using a macro for now.  But there could be others ways to do that
sixtyfps::sixtyfps! {
    SuperSimple = Rectangle {
        color: white;

        Rectangle {
            width: 100;
            height: 100;
            color: blue;
        }
        Rectangle {
            x: 100;
            y: 100;
            width: (100);
            height: {100}
            color: green;
        }
        Image {
            x: 200;
            y: 200;
            source: "../../examples/graphicstest/logo.png";
        }
    }
}

fn main() {
    SuperSimple::default().run();
}
