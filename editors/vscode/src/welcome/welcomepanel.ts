// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial

import * as vscode from "vscode";

const pkg = require("../../package.json");

// This file is the Welcome Panel/Page
export class WelcomePanel {
    static #currentPanel: WelcomePanel | undefined;
    #webview: vscode.WebviewPanel;
    #disposables: vscode.Disposable[] = [];

    constructor(extensionPath: string, column: vscode.ViewColumn) {
        const url = vscode.Uri.file(extensionPath);
        this.#webview = vscode.window.createWebviewPanel(
            "slint.WelcomePage",
            "Slint Welcome Page",
            column,
            {
                enableScripts: true,
                retainContextWhenHidden: true,
                localResourceRoots: [vscode.Uri.joinPath(url, "static")],
            },
        );
        this.getHtml(url).then(
            (contents) => (this.#webview.webview.html = contents),
        );
        this.#webview.webview.onDidReceiveMessage(async (message: any) => {
            switch (message.command) {
                case "ChangeShowWelcomeConfig": {
                    WelcomePanel.showWelcomeOnActivation =
                        WelcomePanel.filterShowWelcomeOnActivation(
                            message.value,
                        );
                    return;
                }
            }
        });

        this.#webview.onDidDispose(
            () => this.dispose(),
            null,
            this.#disposables,
        );
    }

    public dispose() {
        WelcomePanel.#currentPanel = undefined;
        this.#webview.dispose();
    }

    public static createOrShow(extensionPath: string) {
        const column = vscode.window.activeTextEditor?.viewColumn;
        if (WelcomePanel.#currentPanel) {
            WelcomePanel.#currentPanel.#webview.reveal(column);
        } else {
            WelcomePanel.#currentPanel = new WelcomePanel(
                extensionPath,
                column ?? vscode.ViewColumn.One,
            );
        }
        WelcomePanel.updateShowConfig();
    }

    public static maybeShow(extensionPath: string) {
        if (WelcomePanel.openPanelOnActivation()) {
            WelcomePanel.createOrShow(extensionPath);
        }
    }

    public static postMessage(message: any) {
        if (WelcomePanel.#currentPanel) {
            WelcomePanel.#currentPanel.#webview.webview.postMessage(message);
        }
    }

    private static set showWelcomeOnActivation(value: string) {
        vscode.workspace
            .getConfiguration("slint")
            .update("showWelcomeOnActivation", value, true);
    }

    private static get showWelcomeOnActivation(): string {
        return (
            vscode.workspace
                .getConfiguration("slint")
                .get("showWelcomeOnActivation") ?? ""
        );
    }

    private static filterShowWelcomeOnActivation(value: string): string {
        if (value === "always" || value === "never") {
            return value;
        }
        return "never";
    }

    private static openPanelOnActivation(): boolean {
        const result = WelcomePanel.showWelcomeOnActivation;
        const filtered = this.filterShowWelcomeOnActivation(result);
        if (result !== filtered) {
            WelcomePanel.showWelcomeOnActivation = filtered;
        }

        return result !== "never"; // Use the result here, not filtered!
    }

    static updateShowConfig() {
        const value = WelcomePanel.showWelcomeOnActivation;
        WelcomePanel.postMessage({
            command: "ChangeShowWelcomeConfig",
            value: this.filterShowWelcomeOnActivation(value),
        });
    }

    private async getHtml(extensionUri: vscode.Uri): Promise<string> {
        let data = await vscode.workspace.fs.readFile(
            vscode.Uri.joinPath(
                extensionUri,
                "static",
                "welcome",
                "index.html",
            ),
        );
        let result = Buffer.from(data).toString("utf-8");

        let version = `releases/${pkg.version}`;
        if (pkg.name.endsWith("-nightly")) {
            version = `snapshots/master`;
        }
        result = result.replaceAll("<VERSION>", version);
        result = result.replaceAll(
            "<ASSETS_URL>",
            this.#webview.webview
                .asWebviewUri(
                    vscode.Uri.joinPath(extensionUri, "static", "assets"),
                )
                .toString(),
        );

        return result;
    }
}
