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

var editor = monaco.editor.create(document.getElementById("editor"));

function load_from_url(url) {
    fetch(url).then(
        x => x.text().then(y => editor.getModel().setValue(y))
    );

}

load_from_url("https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/gallery/gallery.60");

let select = (<HTMLInputElement>document.getElementById("select_combo"));
select.onchange = function () {
    load_from_url("https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/" + select.value);
};

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
    div.innerHTML = "";
    div.appendChild(canvas);
    try {
        sixtyfps.instantiate_from_string(source, canvas_id);
    } catch (e) {
        if (e.message === "Using exceptions for control flow, don't mind me. This isn't actually an error!") {
            monaco.editor.setModelMarkers(editor.getModel(), "sixtyfps", []);
            throw e;
        }
        var text = document.createTextNode(e.message);
        var p = document.createElement('pre');
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
}

run();