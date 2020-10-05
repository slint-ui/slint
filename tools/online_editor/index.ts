/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
import * as monaco from 'monaco-editor';

var sixtyfps;

async function run() {
    sixtyfps = await import("../../api/sixtyfps-wasm-interpreter/pkg/index.js");
    update();
}

run();


var editor = monaco.editor.create(document.getElementById("editor"), {
    value: `
import { SpinBox, Button, CheckBox, Slider, GroupBox } from "sixtyfps_widgets.60";

Label := Text {
    color: black;
    font_size: 28lx;
}

Page := Rectangle {
    color: white;
    animate y { duration: 300ms; easing: ease_out; }
}

Preview := Rectangle {
    color: pink;
    width: 320lx;
    height: 480lx;
}


CopyPage := Page {

    preview := Preview {
        x: 40lx;
        y: 20lx;
        height: parent.height - 40lx;
    }

    copies_label := Label {
        x: preview.x + preview.width + 20lx;
        y: preview.y + 10lx;
        width: root.width - self.x - 40lx;
        height: 40lx;
        text: "Copies:";
    }

    copies_spinbox := SpinBox {
        x: copies_label.x;
        y: copies_label.y + copies_label.height + 10lx;
        width: copies_label.width;
        height: copies_label.height;
        font_size: 28lx;
    }

    Button {
        text: "Print Page";
        font_size: 28lx;
        x: copies_spinbox.x;
        y: copies_spinbox.y + copies_spinbox.height + 10lx;
        width: copies_spinbox.width;
        height: copies_spinbox.height;
    }


}

FaxPage := Page {


    preview := Preview {
        x: 40lx;
        y: 20lx;
        height: parent.height - 40lx;
    }

    property<int> fax_number: 0;

    text := Text {
        x: preview.x + preview.width + 20lx;
        y: preview.y + 10lx;
        text: fax_number;
        width: parent.width - x - 40lx;
        horizontal_alignment: align_center;
        font_size: 40lx;
        color: black;
        height: 40lx;
    }

    Rectangle {
        color: #333;
        x: text.x;
        y: text.y + text.height + 20lx;
        width: text.width;
        height: send.y - y - 40lx;

        for row_model[r] in [
            [ 7, 8, 9 ],
            [ 4, 5, 6 ],
            [ 1, 2, 3 ],
            [ 0, -1 ],
        ] : Rectangle {
            width: parent.width;
            height: (parent.height - 5*10lx) / 4;
            y: r * (self.height + 10lx) + 10lx;

            for num[c] in row_model : Rectangle {
                height: parent.height;
                width: (parent.width - 4*10lx) / 3;
                x: c * (self.width + 10lx) + 10lx;
                color: key_area.pressed ? #566 : #555 ;
                Text {
                    height: parent.height;
                    width: parent.width;
                    horizontal_alignment: align_center;
                    vertical_alignment: align_center;
                    color: white;
                    text: num >= 0 ? num : "⌫";
                }
                key_area := TouchArea {
                    width: parent.width;
                    height: parent.height;
                    clicked => {
                        if (num >= 0) {
                            fax_number *= 10;
                            fax_number += num;
                        } else {
                            fax_number /= 10;
                        }
                    }
                }
            }
        }
    }

    send := Button {
        text: "Send";
        font_size: 28lx;
        y: parent.height - height - 20lx;
        x: parent.width - width - 40lx;
        /* FIXME: does not work with the native stylr
        width: minimum_width;
        height: minimum_height;
        */
        width: 120lx;
        height: 40lx;
    }
}

PrintPage := Page {

    Rectangle {
        color: #ddd;
        width: parent.width / 3;
        x: parent.width / 12  ;
        y: parent.width / 8;
        height: parent.width / 2;
        Text {
            color: black;
            text: "Hello";
        }
    }
    Rectangle {
        color: #ddd;
        width: parent.width / 3;
        x: parent.width / 12 + parent.width / 2 ;
        y: parent.width / 8;
        height: parent.width / 2;
        Text {
            color: black;
            text: "Print";
        }
    }
}


SettingsPage := Page {
   GridLayout {
        height: parent.height - 20lx;
        width: parent.width - 20lx;
        x: 10lx;
        y: 10lx;
        spacing: 10lx;
        padding: 15lx;
        Row {
            GroupBox {
                title: "Color Management";
                GridLayout {
                    Row {
                        CheckBox {
                            text: "Black and White";
                        }
                    }
                }
            }
        }
        Row {
            GroupBox {
                title: "Scanning";
                GridLayout {
                    Row {
                        Text {
                            text: "Resolution (DPI)";
                            color: black;
                        }
                        Slider {  }
                        Rectangle { }
                    }
                }
            }
        }
        Row {
            GroupBox {
                title: "Power Management";
                GridLayout {
                    Row {
                        CheckBox { text: "Eco Mode"; }
                    }
                }
            }
        }
        Row {
            GroupBox {
                title: "Performance";
                GridLayout {
                    Row {
                        CheckBox { text: "TURBO"; }
                    }
                }
            }
        }
        Row { Rectangle {} }
        Row { Rectangle {} }
        Row { Rectangle {} }
    }
}

TopPanel := Rectangle {
   property<int> active_page: 0;

   color: white;

   title1 := Text {
        text: "PrintMachine";
        color: active_page == 0 ? black : #0000;
        animate color { duration: 200ms; }
        font_size: root.width / 20;
        y: 5lx;
        x: 40lx;
    }
    Text {
        y:  title1.y;
        x: title1.x + title1.font_size*7; //title1.x + title1.width;
        text: "2000";
        color: active_page == 0 ? #918e8c : #0000;
        animate color { duration: 200ms; }
        font_size: root.width / 20;
    }

}

MainWindow := Window {

    width: 800lx;
    height: 600lx;

    /// Note that this property is overwriten in the .cpp and .rs code.
    // The data is only in this file so it looks good in the viewer
    property <[{color: color, level: float}]> ink_levels: [
                {color: #0ff, level: 60%},
                {color: #ff0, level: 80%},
                {color: #f0f, level: 70%},
                {color: #000, level: 30%},
            ];

    property<int> active_page: 0;

    panel := TopPanel {
        active_page: root.active_page;
        width: root.width;
        height: root.width / 20 + 10lx;
    }

    for page_info[idx] in [
        { color: #129c08, text: "Copy", icon: "📋" },
        { color: #009f6f, text: "Fax", icon: "📠" },
        { color: #009ca9, text: "Print", icon: "🖨️" },
        { color: #0093bf, text: "Settings", icon: "⚙️" },
    ] : Rectangle {
        width: root.width / 5;
        height: root.height / 3;
        y: root.height / 4;
        x: idx * root.width / 4 + root.width / 45;
        border_radius: 25lx;
        color: page_info.color;
        img := Text {
            y: 5lx;
            x: parent.border_radius + (self.width / 2) - ((parent.width - parent.border_radius) / 2);
            width: root.width / 6.25;
            height: root.height / 4.68;
            animate x, y, width, height {
                duration: 300ms;
                easing: ease_in_out;
            }
            color: black;
            text: page_info.icon;
        }
        text := Text {
            color: black;
            y: root.height / 4.5;
            x: 5lx;
            width: parent.width;
            height: parent.height;
            horizontal_alignment: align_center;
            text: page_info.text;
            font_size: 28lx;
            animate x, y {
                duration: 300ms;
                easing: ease_in_out;
            }
        }
        touch := TouchArea {
            width: parent.width;
            height: parent.height;
            clicked => {
                if (root.active_page == 0) {
                    root.active_page = idx + 1;
                }
            }
        }

        animate x, y, height, width, color, border_radius {
            duration: 300ms;
            easing: ease_in_out;
        }

        states [
            active when root.active_page == idx + 1: {
                x: 0px;
                y: 0px;
                height: root.height / 8;
                width: root.width;
                border_radius: 0lx;
                img.x: root.height / 8;
                img.width: root.height / 10;
                img.height: root.height / 10;
                text.y: 0px;
                text.x: root.height / 4;
                //text.horizontal_alignment: align_left;
            }
            pressed when root.active_page == 0 && touch.pressed : {
                width: root.width / 5 + 6lx;
                height: root.height / 3 + 6lx ;
                y: root.height / 4 - 3lx;
                x: idx * root.width / 4 + root.width / 45 - 3lx;
                img.x: 8lx;
                img.y: 8lx;
                text.y: root.height / 5 + 5lx;
            }
            invisible when root.active_page > 0 && root.active_page != idx + 1 : {
                color: transparent;
                // FIXME: should probaby hide the entire item under with z-ordering
                img.y: 1000000000lx;
                text.color: #0000;
            }
        ]
    }

    if (root.active_page != 0) : Rectangle {
        width: root.height / 8;
        height: root.height / 8;
        Text {
             text: "←";
             color: white;
             font_size: root.height / 8;
        }
        TouchArea {
            width: root.height / 8;
            height: root.height / 8;
            clicked => { root.active_page = 0; }
        }
    }


    Rectangle {
        width: root.width / 5;
        height: root.height / 5;
        x: root.width - self.width - 20lx;
        y: root.height - self.height - 20lx;
        color: #eee;

        //GridLayout {
        //    spacing: 20lx;
            for color_info[idx] in ink_levels : Rectangle {

                color: white;

    // this should be done by the Layout later
    width: (parent.width - 5*10lx) / 4;
    height: parent.height - 20lx;
    y: 10lx;
    x: 10lx + (self.width + 10lx) * idx;

                Rectangle {
                    width: parent.width;
                    height: tentative_height > parent.height * color_info.level ? parent.height * color_info.level: tentative_height;
                    property <length> tentative_height: root.active_page == 0 ? parent.height * color_info.level : 0px;
                    animate tentative_height { duration: 750ms; easing: ease_in_out; }
                    y: parent.height - self.height;
                    color: color_info.color;
                }
            }
        //}

        property <bool> full_screen;
        states [
            full_screen when full_screen : {
                width: root.width - 35lx;
                height: 7/8 * root.height - 40lx ;
            }
        ]
        animate width, height { duration: 200ms; easing: ease;  }
        TouchArea {
            width: parent.width;
            height: parent.height;
            clicked => {
                if (active_page == 0) {
                    parent.full_screen = !parent.full_screen;
                }
            }
        }
    }

    CopyPage {
        height: root.height - root.height / 8;
        width: root.width;
        y: root.height;
        states [
            active when root.active_page == 1: {
                y: root.height / 8;
            }
        ]
    }

    FaxPage {
        height: root.height - root.height / 8;
        width: root.width;
        y: root.height;
        states [
            active when root.active_page == 2: {
                y: root.height / 8;
            }
        ]
    }

    PrintPage {
        height: root.height - root.height / 8;
        width: root.width;
        y: root.height;
        states [
            active when root.active_page == 3: {
                y: root.height / 8;
            }
        ]
    }

    SettingsPage {
        height: root.height - root.height / 8;
        width: root.width;
        y: root.height;
        states [
            active when root.active_page == 4: {
                y: root.height / 8;
            }
        ]
    }
}
    `,
    //language: "javascript"
});


editor.getModel().onDidChangeContent(function () {
    update();
});

function update() {
    let source = editor.getModel().getValue();
    let div = document.getElementById("preview");
    setTimeout(function () { render_or_error(source, div); }, 1);
}


function render_or_error(source, div) {
    let canvas_id = 'canvas_' + Math.random().toString(36).substr(2, 9);
    let canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 600;
    canvas.id = canvas_id;
    div.appendChild(canvas);
    try {
        sixtyfps.instantiate_from_string(source, canvas_id);
    } catch (e) {
        if (e.message === "Using exceptions for control flow, don't mind me. This isn't actually an error!") {
            throw e;
        }
        var text = document.createTextNode(e.message);
        var p = document.createElement('pre');
        p.appendChild(text);
        div.innerHTML = "<pre style='color: red; background-color:#fee; margin:0'>" + p.innerHTML + "</pre>";

        throw e;
    }
}
