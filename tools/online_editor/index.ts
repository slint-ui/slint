/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
import * as monaco from 'monaco-editor';
import { sixtyfps_language } from "./highlighting";

var sixtyfps;
monaco.languages.register({
    id: 'sixtyfps'
});
monaco.languages.onLanguage('sixtyfps', () => {
    monaco.languages.setMonarchTokensProvider('sixtyfps', sixtyfps_language);
});
var editor = monaco.editor.create(document.getElementById("editor"), {
    language: 'sixtyfps'
});
var base_url = "";

let hello_world = `
import { SpinBox, Button, CheckBox, Slider, GroupBox } from "sixtyfps_widgets.60";
export Demo := Window {
    width: 300px;
    height: 300px;
    t:= Text {
        text: "Hello World";
    }
    Image{
        y: 50px;
        source: img!"https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/resources/logo_scaled.png";
    }
}
`

function load_from_url(url) {
    fetch(url).then(
        x => x.text().then(y => {
            base_url = url;
            editor.getModel().setValue(y)
        })
    );

}

let select = (<HTMLInputElement>document.getElementById("select_combo"));
function select_combo_changed() {
    if (select.value) {
        load_from_url("https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/" + select.value);
    } else {
        base_url = "";
        editor.getModel().setValue(hello_world)
    }
}
select.onchange = select_combo_changed;

let compile_button = (<HTMLButtonElement>document.getElementById("compile_button"));
compile_button.onclick = function () {
    update();
};

let auto_compile = (<HTMLInputElement>document.getElementById("auto_compile"));
auto_compile.onchange = function () {
    if (auto_compile.checked) {
        update()
    }
};

function update() {
    let source = editor.getModel().getValue();
    let div = document.getElementById("preview");
    setTimeout(function () { render_or_error(source, base_url, div); }, 1);
}


async function render_or_error(source, base_url, div) {
    let canvas_id = 'canvas_' + Math.random().toString(36).substr(2, 9);
    let canvas = document.createElement("canvas");
    canvas.width = 800;
    canvas.height = 600;
    canvas.id = canvas_id;
    div.innerHTML = "";
    div.appendChild(canvas);
    try {
        var compiled_component = await sixtyfps.compile_from_string(source, base_url);
    } catch (e) {
        let text = document.createTextNode(e.message);
        let p = document.createElement('pre');
        p.appendChild(text);
        div.innerHTML = "<pre style='color: red; background-color:#fee; margin:0'>" + p.innerHTML + "</pre>";

        if (e.errors) {
            let markers = e.errors.map(function (x) {
                return {
                    severity: 3 - x.level,
                    message: x.message,
                    source: x.fileName,
                    startLineNumber: x.lineNumber,
                    startColumn: x.columnNumber,
                    endLineNumber: x.lineNumber,
                    endColumn: -1,
                }
            });
            monaco.editor.setModelMarkers(editor.getModel(), "sixtyfps", markers);
        }

        throw e;
    }
    compiled_component.run(canvas_id)
}

let keystorke_timeout_handle;

async function run() {
    const params = new URLSearchParams(window.location.search);
    const code = params.get("snippet");
    const load_url = params.get("load_url");
    if (code) {
        editor.getModel().setValue(code);
    } else if (load_url) {
        load_from_url(load_url);
    } else {
        editor.getModel().setValue(hello_world);
    }
    sixtyfps = await import("../../api/sixtyfps-wasm-interpreter/pkg/index.js");
    update();
    editor.getModel().onDidChangeContent(function () {
        let permalink = (<HTMLAnchorElement>document.getElementById("permalink"));
        let params = new URLSearchParams();
        params.set("snippet", editor.getModel().getValue());
        let this_url = new URL(window.location.toString());
        this_url.search = params.toString();
        permalink.href = this_url.toString();
        if (auto_compile.checked) {
            if (keystorke_timeout_handle) {
                clearTimeout(keystorke_timeout_handle);
            }
            keystorke_timeout_handle = setTimeout(update, 500);

        }
    });
}

run();