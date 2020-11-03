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

/// Index by url. Inline documents will use the empty string.
var editor_documents: Map<string, monaco.editor.ITextModel> = new Map;

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

function load_from_url(url: string) {
    clearTabs();
    fetch(url).then(
        x => x.text().then(y => {
            base_url = url;
            let model = createMainModel(y, url);
            addTab(model, url);
        })
    );
}

let select = (<HTMLInputElement>document.getElementById("select_combo"));
function select_combo_changed() {
    if (select.value) {
        load_from_url("https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/" + select.value);
    } else {
        clearTabs();
        base_url = "";
        let model = createMainModel(hello_world, "");
        addTab(model);
    }
}
select.onchange = select_combo_changed;

let compile_button = (<HTMLButtonElement>document.getElementById("compile_button"));
compile_button.onclick = function () {
    update_preview();
};

let auto_compile = (<HTMLInputElement>document.getElementById("auto_compile"));
auto_compile.onchange = function () {
    if (auto_compile.checked) {
        update_preview()
    }
};

function tabTitleFromURL(url: string): string {
    if (url === "") {
        return "unnamed.60";
    }
    try {
        let parsed_url = new URL(url);
        let path = parsed_url.pathname;
        return path.substring(path.lastIndexOf('/') + 1);
    } catch (e) {
        return url;
    }
}

function maybe_update_preview_automatically() {
    if (auto_compile.checked) {
        if (keystorke_timeout_handle) {
            clearTimeout(keystorke_timeout_handle);
        }
        keystorke_timeout_handle = setTimeout(update_preview, 500);
    }
}

function createMainModel(source: string, url: string): monaco.editor.ITextModel {
    let model = monaco.editor.createModel(source);
    model.onDidChangeContent(function () {
        let permalink = (<HTMLAnchorElement>document.getElementById("permalink"));
        let params = new URLSearchParams();
        params.set("snippet", editor.getModel().getValue());
        let this_url = new URL(window.location.toString());
        this_url.search = params.toString();
        permalink.href = this_url.toString();
        maybe_update_preview_automatically();
    });
    editor_documents.set(url, model);
    update_preview();
    return model;
}

function clearTabs() {
    let tab_bar = document.getElementById("tabs") as HTMLUListElement;
    tab_bar.innerHTML = "";
    editor_documents.clear();
}

function addTab(model: monaco.editor.ITextModel, url: string = "") {
    let tab_bar = document.getElementById("tabs") as HTMLUListElement;
    let tab = document.createElement("li");
    tab.setAttribute("class", "nav-item");
    tab.dataset["url"] = url;
    tab.innerHTML = `<span class="nav-link">${tabTitleFromURL(url)}</span>`;
    tab_bar.appendChild(tab);
    $(tab).on("click", (e) => {
        e.preventDefault();
        setCurrentTab(url);
    });
    if (tab_bar.childElementCount == 1) {
        setCurrentTab(url);
    }
}

function setCurrentTab(url: string) {
    let current_tab = document.querySelector(`#tabs li[class~="nav-item"] span[class~="nav-link"][class~="active"]`);
    if (current_tab != undefined) {
        current_tab.className = "nav-link";
    }
    let new_current = document.querySelector(`#tabs li[class~="nav-item"][data-url="${url}"] span[class~="nav-link"]`);
    if (new_current != undefined) {
        new_current.className = "nav-link active";
    }
    let model = editor_documents.get(url);
    if (model != undefined) {
        editor.setModel(model);
    }
}

function update_preview() {
    let main_model = editor_documents.get(base_url);
    if (main_model === undefined) {
        return;
    }
    let source = main_model.getValue();
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
        var compiled_component = await sixtyfps.compile_from_string(source, base_url, (file_name: string) => {
            let u = new URL(file_name, base_url);
            return u.toString();
        }, async (url: string) => {
            console.log("ERR", url);
            let doc = editor_documents.get(url);
            if (doc === undefined) {
                const response = await fetch(url);
                let doc = await response.text();
                let model = monaco.editor.createModel(doc);
                model.onDidChangeContent(function () {
                    maybe_update_preview_automatically();
                });
                editor_documents.set(url, model);
                addTab(model, url);
                return doc;
            }
            return doc.getValue();
        });
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
    sixtyfps = await import("../../api/sixtyfps-wasm-interpreter/pkg/index.js");
    const params = new URLSearchParams(window.location.search);
    const code = params.get("snippet");
    const load_url = params.get("load_url");
    if (code) {
        clearTabs();
        let model = createMainModel(code, "");
        addTab(model);
    } else if (load_url) {
        load_from_url(load_url);
    } else {
        clearTabs();
        base_url = "";
        let model = createMainModel(hello_world, "");
        addTab(model);
    }
}

run();