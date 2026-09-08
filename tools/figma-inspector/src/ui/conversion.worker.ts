// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { CaptureAssetReceiver } from "../asset-transport";
import { isPluginToUiMessage } from "../protocol";
import {
    convertCapture,
    convertExport,
    type CaptureMessage,
    type CaptureConversionState,
    captureSnapshotJson,
} from "../preview/convert-capture";
import {
    isSnapshotRequest,
    isExportRequest,
    type ExportRequest,
} from "../protocol";

const assetCache = new CaptureAssetReceiver();
const captures = new Map<
    number,
    { request: CaptureMessage; state: CaptureConversionState }
>();
const exports = new Map<number, Promise<import("../protocol").ExportPackage>>();

async function handleExport(request: ExportRequest): Promise<void> {
    const capture = captures.get(request.revision);
    if (!capture) {
        self.postMessage({
            kind: "export-result",
            revision: request.revision,
            requestId: request.requestId,
            workerMs: 0,
            exportError: "Export source is no longer available",
        });
        return;
    }
    const started = performance.now();
    let job = exports.get(request.revision);
    if (!job) {
        job = convertExport(capture.request, assetCache, capture.state);
        exports.set(request.revision, job);
        void job
            .catch(() => undefined)
            .finally(() => {
                void job?.then(
                    () => undefined,
                    () => exports.delete(request.revision),
                );
            });
    }
    try {
        const exportPackage = await job;
        self.postMessage({
            kind: "export-result",
            revision: request.revision,
            requestId: request.requestId,
            workerMs: performance.now() - started,
            exportPackage,
        });
    } catch (error) {
        exports.delete(request.revision);
        self.postMessage({
            kind: "export-result",
            revision: request.revision,
            requestId: request.requestId,
            workerMs: performance.now() - started,
            exportError: error instanceof Error ? error.message : String(error),
        });
    }
}

self.onmessage = async (event: MessageEvent<unknown>) => {
    const request = event.data;
    if (isSnapshotRequest(request)) {
        const started = performance.now();
        try {
            const captured = captures.get(request.revision);
            if (!captured)
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
    if (isExportRequest(request)) {
        await handleExport(request);
        return;
    }
    if (!isPluginToUiMessage(request) || request.type !== "preview-capture")
        return;
    const captured = { request, state: {} };
    captures.set(request.revision, captured);
    for (const revision of captures.keys())
        if (revision !== request.revision) {
            captures.delete(revision);
            exports.delete(revision);
        }
    const start = performance.now();
    const message = await convertCapture(request, assetCache, captured.state);
    self.postMessage({
        revision: request.revision,
        message,
        workerMs: performance.now() - start,
    });
};
