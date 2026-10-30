// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { isPluginToUiMessage, isSnapshotReply } from "../protocol";
import type { TimingTrace } from "../performance/timing";
import type { PluginToUiMessage } from "../protocol";
import { exportFailure } from "../preview/export-diagnostic";
import type {
    CaptureMessage,
    ConvertedMessage,
} from "../preview/convert-capture";

export interface ConversionWorker {
    onmessage: ((event: MessageEvent<unknown>) => void) | null;
    onerror: ((event: ErrorEvent) => void) | null;
    onmessageerror: ((event: MessageEvent<unknown>) => void) | null;
    postMessage(value: unknown): void;
    terminate(): void;
}
type ExportMessage = Extract<PluginToUiMessage, { type: "preview-export" }>;
type Slot = {
    capture: CaptureMessage;
    revision: number;
    converted?: Extract<ConvertedMessage, { type: "preview-source" }>;
    exported?: ExportMessage;
    worker?: ConversionWorker;
    ready: boolean;
    presented: boolean;
    snapshotJson?: string;
    pending?: {
        id: number;
        promise: Promise<string>;
        resolve: (json: string) => void;
        reject: (error: Error) => void;
    };
    onPreview: (message: ConvertedMessage, workerMs: number) => void;
    onError: (error: Error) => void;
    onExport?: (message: ExportMessage) => void;
};

export class ConversionClient {
    private current?: Slot;
    private nextRequest = 1;
    private retained = new Map<string, Slot>();
    private preparing?: { revision: number; worker: ConversionWorker };
    public prepareBulk(
        request: Extract<PluginToUiMessage, { type: "preview-bulk" }>,
    ): void {
        try {
            if (this.preparing?.revision !== request.revision) {
                this.preparing?.worker.terminate();
                this.preparing = {
                    revision: request.revision,
                    worker: this.createWorker(),
                };
                const worker = this.preparing.worker;
                const failed = () => {
                    if (this.preparing?.worker === worker) {
                        worker.terminate();
                        this.preparing = undefined;
                    }
                };
                worker.onerror = failed;
                worker.onmessageerror = failed;
            }
            this.preparing.worker.postMessage(request);
        } catch {
            this.preparing?.worker.terminate();
            this.preparing = undefined;
        }
    }
    public reserve(): void {
        this.preparing?.worker.terminate();
        this.preparing = undefined;
        const slot = this.current;
        if (!slot?.converted || !slot.exported || !slot.worker) {
            this.clear();
            return;
        }
        slot.presented = false;
        slot.pending?.reject(new Error("Capture retired"));
        slot.pending = undefined;
    }
    public reuse(
        captureId: string,
        revision: number,
        trace: TimingTrace,
        onPreview: Slot["onPreview"],
        onError: Slot["onError"],
        onExport?: Slot["onExport"],
    ): boolean {
        const slot =
            this.retained.get(captureId) ??
            (this.current?.capture.captureId === captureId
                ? this.current
                : undefined);
        if (!slot?.worker || !slot.converted || !slot.exported) return false;
        this.retained.delete(captureId);
        if (this.current !== slot) this.clear();
        slot.revision = revision;
        slot.presented = false;
        slot.onPreview = onPreview;
        slot.onError = onError;
        slot.onExport = onExport;
        this.current = slot;
        onPreview({ ...slot.converted, revision, trace }, 0);
        onExport?.({ ...slot.exported, revision });
        return true;
    }
    public dispose(): void {
        this.preparing?.worker.terminate();
        this.preparing = undefined;
        if (this.current) this.stop(this.current, new Error("Capture retired"));
        this.current = undefined;
        for (const slot of this.retained.values())
            this.stop(slot, new Error("Capture evicted"));
        this.retained.clear();
    }
    private size(slot: Slot): number {
        const source = slot.converted?.source;
        const sourceSize =
            typeof source === "string"
                ? source.length * 2
                : source
                  ? source.source.reduce(
                        (size: number, part) =>
                            size +
                            (typeof part === "string" ? part.length * 2 : 8),
                        0,
                    ) +
                    source.assets.reduce(
                        (size, asset) => size + asset.length * 2,
                        0,
                    )
                  : 0;
        return (
            slot.capture.captureJson.length * 4 +
            sourceSize +
            (slot.capture.captureAssets ?? []).reduce(
                (size: number, asset) =>
                    size +
                    (asset instanceof Uint8Array ? asset.byteLength * 2 : 0),
                0,
            ) +
            (slot.exported?.exportPackage?.source.length ?? 0) * 2 +
            (slot.exported?.exportPackage?.files ?? []).reduce(
                (size, file) => size + file.data.length * 2,
                0,
            )
        );
    }
    constructor(private readonly createWorker: () => ConversionWorker) {}
    capture(
        capture: CaptureMessage,
        onPreview: Slot["onPreview"],
        onError: Slot["onError"],
        onExport?: Slot["onExport"],
    ): void {
        const prepared = this.preparing;
        this.preparing = undefined;
        this.clear();
        this.current = {
            capture,
            revision: capture.revision,
            ready: false,
            presented: false,
            onPreview,
            onError,
            onExport,
        };
        if (prepared && prepared.revision !== capture.revision)
            prepared.worker.terminate();
        this.start(
            this.current,
            prepared?.revision === capture.revision
                ? prepared.worker
                : undefined,
        );
    }
    present(revision: number): void {
        if (this.current?.revision === revision) this.current.presented = true;
    }
    fail(revision: number): void {
        if (this.current?.revision === revision) this.clear();
    }
    clear(): void {
        this.preparing?.worker.terminate();
        this.preparing = undefined;
        const slot = this.current;
        this.current = undefined;
        if (!slot) return;
        slot.pending?.reject(new Error("Capture retired"));
        slot.pending = undefined;
        if (
            slot.capture.captureId &&
            slot.worker &&
            slot.converted &&
            slot.exported &&
            this.size(slot) <= 128 * 1024 * 1024
        ) {
            this.retained.set(slot.capture.captureId, slot);
            let bytes = [...this.retained.values()].reduce(
                (size, value) => size + this.size(value),
                0,
            );
            while (this.retained.size > 2 || bytes > 128 * 1024 * 1024) {
                const key = this.retained.keys().next().value!;
                const removed = this.retained.get(key)!;
                this.retained.delete(key);
                bytes -= this.size(removed);
                this.stop(removed, new Error("Capture evicted"));
            }
        } else this.stop(slot, new Error("Capture retired"));
    }
    snapshot(revision: number): Promise<string> {
        const slot = this.current;
        if (!slot?.presented || slot.revision !== revision)
            return Promise.reject(new Error("Snapshot source is unavailable"));
        if (slot.snapshotJson !== undefined)
            return Promise.resolve(slot.snapshotJson);
        if (slot.pending) return slot.pending.promise;
        let resolve!: (json: string) => void, reject!: (error: Error) => void;
        const promise = new Promise<string>((a, b) => {
            resolve = a;
            reject = b;
        });
        slot.pending = { id: this.nextRequest++, promise, resolve, reject };
        if (!slot.worker) this.start(slot);
        else if (slot.ready) this.sendSnapshot(slot);
        return promise;
    }
    private sendSnapshot(slot: Slot): void {
        if (!slot.pending) return;
        try {
            slot.worker?.postMessage({
                kind: "snapshot-request",
                revision: slot.capture.revision,
                requestId: slot.pending.id,
            });
        } catch (error) {
            this.stop(
                slot,
                error instanceof Error ? error : new Error(String(error)),
            );
        }
    }
    private stop(slot: Slot, error: Error): void {
        slot.worker?.terminate();
        slot.worker = undefined;
        slot.ready = false;
        slot.pending?.reject(error);
        slot.pending = undefined;
    }
    private start(slot: Slot, prepared?: ConversionWorker): void {
        const failed = (error: Error) => {
            const previewReady = slot.ready;
            this.stop(slot, error);
            if (
                slot.capture.captureId &&
                this.retained.get(slot.capture.captureId) === slot
            )
                this.retained.delete(slot.capture.captureId);
            if (this.current === slot) {
                if (!previewReady) slot.onError(error);
                else
                    slot.onExport?.({
                        type: "preview-export",
                        revision: slot.capture.revision,
                        warnings: [exportFailure(error)],
                    });
            }
        };
        try {
            const worker = prepared ?? this.createWorker();
            slot.worker = worker;
            worker.onerror = () => {
                if (slot.worker === worker)
                    failed(new Error("Conversion worker failed"));
            };
            worker.onmessageerror = () => {
                if (slot.worker === worker)
                    failed(new Error("Invalid conversion worker message"));
            };
            worker.onmessage = (event) => {
                if (this.current !== slot || slot.worker !== worker) return;
                try {
                    const reply = event.data;
                    if (
                        reply &&
                        typeof reply === "object" &&
                        "kind" in reply &&
                        reply.kind === "snapshot-result"
                    ) {
                        if (!isSnapshotReply(reply)) {
                            failed(new Error("Invalid snapshot result"));
                            return;
                        }
                        if (
                            reply.revision !== slot.capture.revision ||
                            reply.requestId !== slot.pending?.id
                        )
                            return;
                        const pending = slot.pending;
                        slot.pending = undefined;
                        if (reply.snapshotJson !== undefined) {
                            slot.snapshotJson = reply.snapshotJson;
                            pending.resolve(reply.snapshotJson);
                        } else pending.reject(new Error(reply.snapshotError));
                        return;
                    }
                    const response = reply as {
                        revision?: unknown;
                        workerMs?: unknown;
                        message?: unknown;
                    } | null;
                    if (
                        !response ||
                        response.revision !== slot.capture.revision ||
                        typeof response.workerMs !== "number" ||
                        !Number.isFinite(response.workerMs) ||
                        response.workerMs < 0 ||
                        !isPluginToUiMessage(response.message) ||
                        ![
                            "preview-source",
                            "preview-clear",
                            "preview-diagnostics",
                            "preview-export",
                        ].includes(response.message.type) ||
                        !("revision" in response.message) ||
                        response.message.revision !== slot.capture.revision
                    ) {
                        failed(new Error("Invalid preview result"));
                        return;
                    }
                    if (response.message.type === "preview-export") {
                        slot.exported = response.message;
                        slot.onExport?.({
                            ...response.message,
                            revision: slot.revision,
                        });
                        return;
                    }
                    const message = response.message as ConvertedMessage;
                    slot.ready = message.type === "preview-source";
                    if (message.type === "preview-source")
                        slot.converted = message;
                    if (!slot.presented)
                        slot.onPreview(
                            { ...message, revision: slot.revision },
                            response.workerMs,
                        );
                    if (slot.pending) {
                        if (slot.ready) this.sendSnapshot(slot);
                        else
                            this.stop(
                                slot,
                                new Error("Cannot prepare snapshot"),
                            );
                    }
                } catch (error) {
                    failed(
                        error instanceof Error
                            ? error
                            : new Error(String(error)),
                    );
                }
            };
            worker.postMessage(slot.capture);
        } catch (error) {
            failed(error instanceof Error ? error : new Error(String(error)));
        }
    }
}
