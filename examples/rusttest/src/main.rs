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
    property<bool> pressed: button_area.pressed;
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
    color: { button_area.pressed ? red : #5898; }
    animate color { duration: 500; }
    animate x {
        duration: 200;
    }
}

Hello := Rectangle {

    for x[idx] in counter: Rectangle {
        color: #8005;
        x: idx * 100;
        width: 75;
        height: 75;
        Rectangle {
            color: #00f5;
            width: 25;
            height: 25;
            x: 25;
            y: 25;
        }
    }


    signal foobar;
    signal plus_clicked;
    signal minus_clicked;
    property<int32> counter : 3;

    color: white;

    if (counter <= 4) :  Rectangle {
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

    plus_button := ButtonRectangle {
        color: blue;
        x: { plus_button.pressed ? 100 : 50; }
        y: 225;
        clicked => { counter += 1 }
        button_text: "+";
    }
    counter_label := Text { x: 100; y: 300; text: counter; color: black; }
    minus_button := ButtonRectangle {
        color: yellow;
        x: { minus_button.pressed ? 100 : 50; }
        y: 350;
        clicked => { minus_clicked() }
        button_text: "-";
    }


    Path {
        x: 100;
        y: 300;
        fill_color: green;
        LineTo {
            x: 100;
            y: 50;
        }
        LineTo {
            x: 0;
            y: 100;
        }
        Close {}
    }

}

}

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
