// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

import { Uri, TextDocumentShowOptions } from "vscode";
import * as vscode from "vscode";
import { BaseLanguageClient } from "vscode-languageclient";
import { URI } from "vscode-languageserver";

let previewPanel: vscode.WebviewPanel | null = null;
let previewUrl: Uri | null = null;
let previewAccessedFiles = new Set();
let previewComponent: string = "";
let queuedPreviewMsg: any | null = null;
let previewBusy = false;
let uriMapping = new Map<string, string>();

/// Initialize the callback on the client to make the web preview work
export function initClientForPreview(client: BaseLanguageClient) {
    client.onRequest("slint/preview_message", async (msg: any) => {
        if (previewPanel) {
            // map urls to webview URL
            if (msg.command === "highlight") {
                msg.data.path = previewPanel.webview
                    .asWebviewUri(Uri.parse(msg.data.path, true))
                    .toString();
            }
            previewPanel.webview.postMessage(msg);
        }
        return;
    });
}

function urlConvertToWebview(webview: vscode.Webview, url: Uri): Uri {
    let webview_uri = webview.asWebviewUri(url);
    uriMapping.set(webview_uri.toString(), url.toString());
    return webview_uri;
}

function reload_preview(url: Uri, content: string, component: string) {
    if (!previewPanel) {
        return;
    }
    if (component) {
        content +=
            "\nexport component _Preview inherits " + component + " {}\n";
    }
    previewAccessedFiles.clear();
    uriMapping.clear();

    let webview_uri = urlConvertToWebview(previewPanel.webview, url).toString();
    previewAccessedFiles.add(webview_uri);
    const style = vscode.workspace
        .getConfiguration("slint")
        .get<[string]>("preview.style");
    const msg = {
        command: "preview",
        base_url: url.toString(),
        webview_uri: webview_uri,
        component: component,
        content: content,
        style: style,
    };
    if (previewBusy) {
        queuedPreviewMsg = msg;
    } else {
        previewPanel.webview.postMessage(msg);
        previewBusy = true;
    }
}

export async function refreshPreview(event?: vscode.TextDocumentChangeEvent) {
    if (!previewPanel || !previewUrl) {
        return;
    }
    if (
        event &&
        !previewAccessedFiles.has(
            urlConvertToWebview(
                previewPanel.webview,
                event.document.uri,
            ).toString(),
        )
    ) {
        return;
    }

    let content_str;
    if (event && event.document.uri === previewUrl) {
        content_str = event.document.getText();
        if (event.document.languageId === "rust") {
            content_str = extract_rust_macro(content_str);
        }
    } else {
        content_str = await getDocumentSource(previewUrl);
    }
    reload_preview(previewUrl, content_str, previewComponent);
}

/// Show the preview for the given path and component
export async function toggleDesignMode() {
    previewPanel?.webview.postMessage({
        command: "toggle_design_mode",
    });
}

/// Show the preview for the given path and component
export async function showPreview(
    context: vscode.ExtensionContext,
    url: Uri,
    component: string,
) {
    previewUrl = url;
    previewComponent = component;

    if (previewPanel) {
        previewPanel.reveal(vscode.ViewColumn.Beside);
    } else {
        // Create and show a new webview
        const panel = vscode.window.createWebviewPanel(
            "slint-preview",
            "Slint Preview",
            vscode.ViewColumn.Beside,
            { enableScripts: true, retainContextWhenHidden: true },
        );
        initPreviewPanel(context, panel);
    }

    let content_str = await getDocumentSource(url);
    reload_preview(url, content_str, previewComponent);
}

async function getDocumentSource(url: Uri): Promise<string> {
    // FIXME: is there a faster way to get the document
    let x = vscode.workspace.textDocuments.find((d) => d.uri === url);
    let source;
    if (x) {
        source = x.getText();
        if (x.languageId === "rust") {
            source = extract_rust_macro(source);
        }
    } else {
        source = new TextDecoder().decode(
            await vscode.workspace.fs.readFile(url),
        );
        if (url.path.endsWith(".rs")) {
            source = extract_rust_macro(source);
        }
    }
    return source;
}

function extract_rust_macro(source: string): string {
    let match;
    const re = /slint!\s*([\{\(\[])/g;

    let last = 0;
    let result = "";

    while ((match = re.exec(source)) !== null) {
        let start = match.index + match[0].length;
        let end = source.length;
        let level = 0;
        let open = match[1];
        let close;
        switch (open) {
            case "(":
                close = ")";
                break;
            case "{":
                close = "}";
                break;
            case "[":
                close = "]";
                break;
        }
        for (let i = start; i < source.length; i++) {
            if (source.charAt(i) === open) {
                level++;
            } else if (source.charAt(i) === close) {
                level--;
                if (level < 0) {
                    end = i;
                    break;
                }
            }
        }

        result += source.slice(last, start).replace(/[^\n]/g, " ");
        result += source.slice(start, end);
        last = end;
    }
    result += source.slice(last).replace(/[^\n]/g, " ");
    return result;
}

function getPreviewHtml(slint_wasm_interpreter_url: Uri): string {
    return `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Slint Preview</title>
    <script type="module">
    "use strict";
    import * as slint from '${slint_wasm_interpreter_url}';
    await slint.default();

    const vscode = acquireVsCodeApi();
    let promises = {};
    let current_instance = null;
    let design_mode = false;

    async function load_file(url) {
        let promise = new Promise(resolve => {
            promises[url] = resolve;
        });
        vscode.postMessage({ command: 'load_file', url: url });
        let from_editor = await promise;
        return from_editor || await (await fetch(url)).text();
    }

    async function element_selected(url, sl, sc, el, ec) {
        vscode.postMessage({ command: 'element_selected',  data: { start: { line: sl, column: sc }, end: { line: el, column: ec }, url: url }});
    }

    async function render(source, base_url, style) {
        let { component, error_string } =
            style ? await slint.compile_from_string_with_style(source, base_url, style, async(url) => await load_file(url))
                  : await slint.compile_from_string(source, base_url, async(url) => await load_file(url));
        if (error_string != "") {
            var text = document.createTextNode(error_string);
            var p = document.createElement('pre');
            p.appendChild(text);
            document.getElementById("slint_error_div").innerHTML = "<pre style='color: red; background-color:#fee; margin:0'>" + p.innerHTML + "</pre>";
        }
        vscode.postMessage({ command: 'preview_ready' });
        if (component !== undefined) {
            document.getElementById("slint_error_div").innerHTML = "";
            if (current_instance !== null) {
                current_instance = component.create_with_existing_window(await current_instance);
            } else {
                try {
                    slint.run_event_loop();
                } catch (e) {
                    // ignore winit event loop exception
                }
                current_instance = (async () => {
                    let new_instance = await component.create("slint_canvas");
                    await new_instance.show();
                    return new_instance;
                })();
            }
            if (current_instance !== null) {
                (await current_instance).set_design_mode(design_mode);
                (await current_instance).on_element_selected(element_selected);    
            }
        }
    }

    window.addEventListener('message', async event => {
        if (event.data.command === "preview") {
            design_mode = event.data.design_mode;
            vscode.setState({base_url: event.data.base_url, component: event.data.component});
            await render(event.data.content, event.data.webview_uri, event.data.style);
        } else if (event.data.command === "file_loaded") {
            let resolve = promises[event.data.url];
            if (resolve) {
                delete promises[event.data.url];
                resolve(event.data.content);
            }
        } else if (event.data.command === "highlight") {
            if (current_instance) {
                (await current_instance).highlight(event.data.data.path, event.data.data.offset);
            }
        } else if (event.data.command === "toggle_design_mode") {
            design_mode = !design_mode;
            if (current_instance != null) {
                (await current_instance).set_design_mode(design_mode);
                (await current_instance).on_element_selected(element_selected);    
            }
        }
    });

    vscode.postMessage({ command: 'preview_ready' });
    </script>
</head>
<body>
  <div id="slint_error_div"></div>
  <canvas id="slint_canvas"></canvas>
</body>
</html>`;
}

export class PreviewSerializer implements vscode.WebviewPanelSerializer {
    context: vscode.ExtensionContext;
    constructor(context: vscode.ExtensionContext) {
        this.context = context;
    }
    async deserializeWebviewPanel(
        webviewPanel: vscode.WebviewPanel,
        state: any,
    ) {
        initPreviewPanel(this.context, webviewPanel);
        previewUrl = Uri.parse(state.base_url, true);

        if (previewUrl) {
            let content_str = await getDocumentSource(previewUrl);
            previewComponent = state.component ?? "";
            reload_preview(previewUrl, content_str, previewComponent);
        }
    }
}

function initPreviewPanel(
    context: vscode.ExtensionContext,
    panel: vscode.WebviewPanel,
) {
    previewPanel = panel;
    // we will get a preview_ready when the html is loaded and message are ready to be sent
    previewBusy = true;
    panel.webview.onDidReceiveMessage(
        async (message) => {
            switch (message.command) {
                case "load_file":
                    let canonical = Uri.parse(message.url, true).toString();
                    previewAccessedFiles.add(canonical);
                    let content_str = undefined;
                    let x = vscode.workspace.textDocuments.find(
                        (d) =>
                            urlConvertToWebview(
                                panel.webview,
                                d.uri,
                            ).toString() === canonical,
                    );
                    if (x) {
                        content_str = x.getText();
                    }
                    panel.webview.postMessage({
                        command: "file_loaded",
                        url: message.url,
                        content: content_str,
                    });
                    return;
                case "preview_ready":
                    if (queuedPreviewMsg) {
                        panel.webview.postMessage(queuedPreviewMsg);
                        queuedPreviewMsg = null;
                    } else {
                        previewBusy = false;
                    }
                    return;
                case "element_selected": {
                    const d = message.data;

                    const inside_uri = Uri.parse(d.url);
                    const range = new vscode.Range(
                        new vscode.Position(
                            d.start.line - 1,
                            d.start.column - 1,
                        ),
                        new vscode.Position(
                            d.start.line - 1,
                            d.start.column - 1,
                        ), // Do not use range!
                    );
                    const outside_uri = Uri.parse(
                        uriMapping.get(d.url) ??
                        Uri.file(inside_uri.fsPath).toString(),
                    );
                    if (outside_uri.scheme !== "invalid") {
                        vscode.window.showTextDocument(outside_uri, {
                            selection: range,
                            preserveFocus: false,
                        } as TextDocumentShowOptions);
                    }
                    return;
                }
            }
        },
        undefined,
        context.subscriptions,
    );
    let slint_wasm_interpreter_url = panel.webview.asWebviewUri(
        Uri.joinPath(context.extensionUri, "out/slint_wasm_interpreter.js"),
    );
    panel.webview.html = getPreviewHtml(slint_wasm_interpreter_url);
    panel.onDidDispose(
        () => {
            previewPanel = null;
        },
        undefined,
        context.subscriptions,
    );
}
