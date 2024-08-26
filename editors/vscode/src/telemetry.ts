// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import * as vscode from "vscode";

export class SlintTelemetrySender implements vscode.TelemetrySender {
    sendEventData(eventName: string, data?: Record<string, any>): void {
        if (!data) {
            return;
        }

        let dataToSend = {
            eventName: eventName,
            data: data,
        };

        fetch("https://slint.dev//telemetry/v1/vscode-usage", {
            headers: {
                "Content-Type": "application/json",
            },
            method: "POST",
            body: JSON.stringify(dataToSend)
        })
    }

    sendErrorData(error: Error, data?: Record<string, any>): void {
        // Nothing to send at the moment
    }
}
