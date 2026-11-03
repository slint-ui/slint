// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { isPluginToUiMessage, isSnapshotReply } from "../protocol";
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
type Slot = {
    capture: CaptureMessage;
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
};

export class ConversionClient {
    private current?: Slot;
    private nextRequest = 1;
    constructor(private readonly createWorker: () => ConversionWorker) {}
    capture(
        capture: CaptureMessage,
        onPreview: Slot["onPreview"],
        onError: Slot["onError"],
    ): void {
        this.clear();
        this.current = {
            capture,
            ready: false,
            presented: false,
            onPreview,
            onError,
        };
        this.start(this.current);
    }
    present(revision: number): void {
        if (this.current?.capture.revision === revision)
            this.current.presented = true;
    }
    fail(revision: number): void {
        if (this.current?.capture.revision === revision) this.clear();
    }
    clear(): void {
        if (this.current) this.stop(this.current, new Error("Capture retired"));
        this.current = undefined;
    }
    snapshot(revision: number): Promise<string> {
        const slot = this.current;
        if (!slot?.presented || slot.capture.revision !== revision)
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
    private start(slot: Slot): void {
        const failed = (error: Error) => {
            this.stop(slot, error);
            if (this.current === slot && !slot.presented) slot.onError(error);
        };
        try {
            const worker = this.createWorker();
            slot.worker = worker;
            worker.onerror = () => {
                if (this.current === slot && slot.worker === worker)
                    failed(new Error("Conversion worker failed"));
            };
            worker.onmessageerror = () => {
                if (this.current === slot && slot.worker === worker)
                    failed(new Error("Invalid conversion worker message"));
            };
            worker.onmessage = (event) => {
                if (this.current !== slot || slot.worker !== worker) return;
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
                    ].includes(response.message.type) ||
                    !("revision" in response.message) ||
                    response.message.revision !== slot.capture.revision
                ) {
                    failed(new Error("Invalid preview result"));
                    return;
                }
                const message = response.message as ConvertedMessage;
                slot.ready = message.type === "preview-source";
                if (!slot.presented) slot.onPreview(message, response.workerMs);
                if (slot.pending) {
                    if (slot.ready) this.sendSnapshot(slot);
                    else this.stop(slot, new Error("Cannot prepare snapshot"));
                }
            };
            worker.postMessage(slot.capture);
        } catch (error) {
            failed(error instanceof Error ? error : new Error(String(error)));
        }
    }
}
