// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import * as vscode from "vscode";

export class SlintTelemetrySender implements vscode.TelemetrySender {
    #telemetry_host: string;

    constructor(extensionMode: vscode.ExtensionMode) {
        if (extensionMode === vscode.ExtensionMode.Production) {
            this.#telemetry_host = "slint.dev";
        } else {
            this.#telemetry_host = "staging.slint.dev";
        }
    }
    sendEventData(eventName: string, data?: Record<string, any>): void {
        if (!data) {
            return;
        }

        const dataToSend = {
            eventName: eventName,
            data: data,
        };

        void fetch(
            `https://${this.#telemetry_host}/telemetry/v1/vscode-usage`,
            {
                headers: {
                    "Content-Type": "application/json",
                },
                method: "POST",
                body: JSON.stringify(dataToSend),
            },
        );
    }

    sendErrorData(error: Error, data?: Record<string, any>): void {
        // Nothing to send at the moment
    }
}
