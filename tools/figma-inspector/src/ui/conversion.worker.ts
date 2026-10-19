// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { indexBulkDocument } from "../plugin/bulk-source";
import { isPluginToUiMessage, isSnapshotRequest } from "../protocol";
import {
    convertCapture,
    captureSnapshotJson,
    type CaptureConversionState,
} from "../preview/convert-capture";

let bulkRevision: number | undefined;
let bulkNodes = new Map<string, Record<string, unknown>>();
let bulkDocuments = new Set<string>();
let bulkIndexMs = 0;
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
    if (isPluginToUiMessage(request) && request.type === "preview-bulk") {
        if (bulkRevision !== request.revision) {
            bulkRevision = request.revision;
            bulkNodes = new Map();
            bulkDocuments = new Set();
            bulkIndexMs = 0;
        }
        const started = performance.now();
        try {
            indexBulkDocument(request.document, bulkNodes);
            bulkDocuments.add(String(request.document.id));
        } catch {
            bulkNodes.clear();
            bulkDocuments.clear();
        } finally {
            bulkIndexMs += performance.now() - started;
        }
        return;
    }
    if (!isPluginToUiMessage(request) || request.type !== "preview-capture")
        return;
    captured = {
        revision: request.revision,
        state: {
            bulkNodes:
                bulkRevision === request.revision ? bulkNodes : undefined,
            bulkDocuments:
                bulkRevision === request.revision ? bulkDocuments : undefined,
            bulkIndexMs: bulkRevision === request.revision ? bulkIndexMs : 0,
        },
    };
    const started = performance.now();
    let published = false;
    const message = await convertCapture(request, captured.state, (preview) => {
        published = true;
        self.postMessage({
            revision: request.revision,
            message: preview,
            workerMs: performance.now() - started,
        });
    });
    self.postMessage({
        revision: request.revision,
        message:
            published && message.type === "preview-source"
                ? {
                      type: "preview-export",
                      revision: request.revision,
                      exportPackage: message.exportPackage,
                      warnings: message.warnings ?? [],
                  }
                : message,
        workerMs: performance.now() - started,
    });
};
