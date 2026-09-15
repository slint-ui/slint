// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { isPluginToUiMessage, isSnapshotRequest } from "../protocol";
import {
    convertCapture,
    captureSnapshotJson,
    type CaptureConversionState,
} from "../preview/convert-capture";

let captured: { revision: number; state: CaptureConversionState } | undefined;
self.onmessage = async (event: MessageEvent<unknown>) => {
    const request = event.data;
    if (isSnapshotRequest(request)) {
        const started = performance.now();
        try {
            if (captured?.revision !== request.revision)
                throw Error("Snapshot source is no longer available");
            const snapshotJson = captureSnapshotJson(captured.state);
            self.postMessage({
                kind: "snapshot-result",
                revision: request.revision,
                requestId: request.requestId,
                workerMs: performance.now() - started,
                snapshotJson,
            });
        } catch (error) {
            self.postMessage({
                kind: "snapshot-result",
                revision: request.revision,
                requestId: request.requestId,
                workerMs: performance.now() - started,
                snapshotError:
                    error instanceof Error ? error.message : String(error),
            });
        }
        return;
    }
    if (!isPluginToUiMessage(request) || request.type !== "preview-capture")
        return;
    captured = { revision: request.revision, state: {} };
    const started = performance.now();
    const message = await convertCapture(request, captured.state);
    self.postMessage({
        revision: request.revision,
        message,
        workerMs: performance.now() - started,
    });
};
