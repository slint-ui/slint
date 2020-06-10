// Using a macro for now.  But there could be others ways to do that
sixtyfps::sixtyfps! {

component TwoRectangle := Rectangle {

    signal clicked;

    Rectangle {
        x: 50;
        y: 50.;
        width: 25;
        height: 25;
        color: red;

        my_area := TouchArea {
            width: 25;
            height: 25;
            clicked => { root.clicked() }
        }

    }
}


component ButtonRectangle := Rectangle {
    property<string> button_text;
    signal clicked;
    width: 100;
    height: 75;
    button_area := TouchArea {
        width: 100;
        height: 75;
        clicked => { root.clicked() }
    }
    Text {
        x: 50;
        y: 10;
        text: button_text;
        color: black;
    }
    color: { button_area.pressed ? red : green; }
}

Hello := Rectangle {

    signal foobar;
    signal plus_clicked;
    signal minus_clicked;
    property<int32> counter;

    color: white;

    TwoRectangle {
        width: 100;
        height: 100;
        color: blue;
        clicked => { foobar() }
    }
    Rectangle {
        x: 100;
        y: 100;
        width: (100);
        height: {100}
        color: green;
        Rectangle {
            x: 50;
            y: 50.;
            width: 25;
            height: 25;
            color: yellow;
        }
    }
    Image {
        x: 200;
        y: 200;
        source: img!"../graphicstest/logo.png";
    }

    ButtonRectangle {
        color: 4289374890;
        x: 50;
        y: 225;
        clicked => { counter += 1 }
        button_text: "+";
    }
    counter_label := Text { x: 100; y: 300; text: counter; color: black; }
    ButtonRectangle {
        color: 4289374890;
        x: 50;
        y: 350;
        clicked => { minus_clicked() }
        button_text: "-";
    }

}

}

fn main() {
    let mut app = Hello::default();

    app.plus_clicked.set_handler(|context, ()| {
        let app = context.component.downcast::<Hello>().unwrap();
        app.counter.set(app.counter.get(context) + 1);
    });
    app.minus_clicked.set_handler(|context, ()| {
        let app = context.component.downcast::<Hello>().unwrap();
        app.counter.set(app.counter.get(context) - 1);
    });

    app.run();
}
