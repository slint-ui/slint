/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
// Using a macro for now.  But there could be others ways to do that
sixtyfps::sixtyfps! {

component TwoRectangle := Rectangle {

    signal clicked;

    Rectangle {
        x: 50px;
        y: 50px;
        width: 25px;
        height: 25px;
        color: red;

        my_area := TouchArea {
            width: 25px;
            height: 25px;
            clicked => { root.clicked() }
        }

    }
}


component ButtonRectangle := Rectangle {
    property<string> button_text;
    property<bool> pressed: button_area.pressed;
    signal clicked;
    width: 100px;
    height: 75px;
    button_area := TouchArea {
        width: 100px;
        height: 75px;
        clicked => { root.clicked() }
    }
    Text {
        x: 50px;
        y: 10px;
        text: button_text;
        color: black;
    }
    color: { button_area.pressed ? red : #5898; }
    animate color { duration: 500ms; }
    animate x {
        duration: 200ms;
    }
}

Hello := Rectangle {


    PathLayout {
        x: 100px;
        y: 300px;
        LineTo {
            x: 100;
            y: 50;
        }
        LineTo {
            x: 0;
            y: 100;
        }
        Close {}

        for x[idx] in counter: Rectangle {
            color: #8005;
            x: idx * 100px;
            width: 75px;
            height: 75px;
            Rectangle {
                color: #00f5;
                width: 25px;
                height: 25px;
                x: 25px;
                y: 25px;
            }
        }
    }


    signal foobar;
    signal plus_clicked;
    signal minus_clicked;
    property<int> counter : 3;

    color: white;

    if (counter <= 4) :  Rectangle {
        x: 100px;
        y: 100px;
        width: (100px);
        height: {100px}
        color: green;
        Rectangle {
            x: 50px;
            y: 50px;
            width: 25px;
            height: 25px;
            color: yellow;
        }
    }
    Image {
        x: 200px;
        y: 200px;
        source: img!"../../resources/logo_scaled.png";
    }

    plus_button := ButtonRectangle {
        color: blue;
        x: { plus_button.pressed ? 100px : 50px; }
        y: 225px;
        clicked => { counter += 1 }
        button_text: "+";
    }
    counter_label := Text { x: 100px; y: 300px; text: counter; color: black; }
    minus_button := ButtonRectangle {
        color: yellow;
        x: { minus_button.pressed ? 100px : 50px; }
        y: 350px;
        clicked => { minus_clicked() }
        button_text: "-";
    }


    Path {
        x: 100px;
        y: 300px;
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
    let app_weak = app.clone().as_weak();
    app.as_ref().on_plus_clicked(move || {
        let app = app_weak.upgrade().unwrap();
        app.as_ref().set_counter(app.as_ref().get_counter() + 1);
    });
    let app_weak = app.clone().as_weak();
    app.as_ref().on_minus_clicked(move || {
        let app = app_weak.upgrade().unwrap();
        app.as_ref().set_counter(app.as_ref().get_counter() - 1);
    });
    app.run();
}
