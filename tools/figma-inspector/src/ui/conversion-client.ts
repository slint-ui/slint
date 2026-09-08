// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import type { ExportPackage } from "../protocol";
import { isPluginToUiMessage } from "../protocol";
import type {
    CaptureMessage,
    ConvertedMessage,
} from "../preview/convert-capture";
import { isExportReply, isSnapshotReply } from "../protocol";

export interface ConversionWorker {
    onmessage: ((event: MessageEvent<unknown>) => void) | null;
    onerror: ((event: ErrorEvent) => void) | null;
    onmessageerror: ((event: MessageEvent<unknown>) => void) | null;
    postMessage(value: unknown): void;
    terminate(): void;
}
type Pending<T = ExportPackage> = {
    id: number;
    promise: Promise<T>;
    resolve: (value: T) => void;
    reject: (error: Error) => void;
};
type Slot = {
    capture: CaptureMessage;
    worker?: ConversionWorker;
    ready: boolean;
    pending?: Pending;
    snapshotPending?: Pending<string>;
    snapshotJson?: string;
    package?: ExportPackage;
    onPreview: (message: ConvertedMessage, workerMs: number) => void;
    onError: (error: Error) => void;
};

/** Owns at most a candidate and the last successfully presented capture.
 * Retained input lets an interrupted export be retried without asking Figma. */
export class ConversionClient {
    private candidate?: Slot;
    private presented?: Slot;
    private nextRequest = 1;
    constructor(private readonly createWorker: () => ConversionWorker) {}
    has(revision: number): boolean {
        return this.find(revision) !== undefined;
    }
    private find(revision: number): Slot | undefined {
        return [this.candidate, this.presented].find(
            (slot) => slot?.capture.revision === revision,
        );
    }
    capture(
        capture: CaptureMessage,
        onPreview: Slot["onPreview"],
        onError: Slot["onError"],
    ): void {
        if (this.candidate) this.retire(this.candidate);
        // Stop an in-flight old export, retaining its captured input for recovery.
        if (
            this.presented &&
            (this.presented.pending || this.presented.snapshotPending)
        )
            this.stop(this.presented, new Error("Selection changed"));
        this.candidate = { capture, ready: false, onPreview, onError };
        this.start(this.candidate);
    }
    present(revision: number): void {
        if (this.candidate?.capture.revision !== revision) return;
        if (this.presented) this.retire(this.presented);
        this.presented = this.candidate;
        this.candidate = undefined;
    }
    fail(revision: number): void {
        if (this.candidate?.capture.revision !== revision) return;
        this.retire(this.candidate);
        this.candidate = undefined;
    }
    clear(): void {
        if (this.candidate) this.retire(this.candidate);
        if (this.presented) this.retire(this.presented);
        this.candidate = undefined;
        this.presented = undefined;
    }
    export(revision: number): Promise<ExportPackage> {
        const slot = this.find(revision);
        if (!slot || slot !== this.presented)
            return Promise.reject(new Error("Export source is unavailable"));
        if (slot.package) return Promise.resolve(slot.package);
        if (slot.pending) return slot.pending.promise;
        let resolve!: Pending["resolve"], reject!: Pending["reject"];
        const promise = new Promise<ExportPackage>((a, b) => {
            resolve = a;
            reject = b;
        });
        slot.pending = { id: this.nextRequest++, promise, resolve, reject };
        if (!slot.worker) this.start(slot);
        else if (slot.ready) this.sendExport(slot);
        return promise;
    }
    snapshot(revision: number): Promise<string> {
        const slot = this.find(revision);
        if (!slot || slot !== this.presented)
            return Promise.reject(new Error("Snapshot source is unavailable"));
        if (slot.snapshotJson !== undefined)
            return Promise.resolve(slot.snapshotJson);
        if (slot.snapshotPending) return slot.snapshotPending.promise;
        let resolve!: Pending<string>["resolve"],
            reject!: Pending<string>["reject"];
        const promise = new Promise<string>((a, b) => {
            resolve = a;
            reject = b;
        });
        slot.snapshotPending = {
            id: this.nextRequest++,
            promise,
            resolve,
            reject,
        };
        if (!slot.worker) this.start(slot);
        else if (slot.ready) this.sendSnapshot(slot);
        return promise;
    }
    private sendSnapshot(slot: Slot): void {
        if (!slot.snapshotPending) return;
        try {
            slot.worker?.postMessage({
                kind: "snapshot-request",
                revision: slot.capture.revision,
                requestId: slot.snapshotPending.id,
            });
        } catch (error) {
            this.stop(
                slot,
                error instanceof Error ? error : new Error(String(error)),
            );
        }
    }
    private sendExport(slot: Slot): void {
        if (!slot.pending) return;
        try {
            slot.worker?.postMessage({
                kind: "export-request",
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
        slot.snapshotPending?.reject(error);
        slot.snapshotPending = undefined;
    }
    private retire(slot: Slot): void {
        this.stop(slot, new Error("Capture retired"));
    }
    private start(slot: Slot): void {
        try {
            const worker = this.createWorker();
            slot.worker = worker;
            const failed = (error: Error) => {
                this.stop(slot, error);
                if (slot === this.candidate) slot.onError(error);
            };
            worker.onerror = () =>
                failed(new Error("Conversion worker failed"));
            worker.onmessageerror = () =>
                failed(new Error("Invalid conversion worker message"));
            worker.onmessage = (event) => {
                if (slot.worker !== worker) return;
                const reply = event.data;
                if (
                    reply &&
                    typeof reply === "object" &&
                    "kind" in reply &&
                    reply.kind === "export-result"
                ) {
                    if (!isExportReply(reply)) {
                        failed(new Error("Invalid export result"));
                        return;
                    }
                    if (
                        reply.revision !== slot.capture.revision ||
                        reply.requestId !== slot.pending?.id
                    )
                        return;
                    const pending = slot.pending;
                    slot.pending = undefined;
                    if (reply.exportPackage) {
                        slot.package = reply.exportPackage;
                        pending.resolve(reply.exportPackage);
                    } else pending.reject(new Error(reply.exportError));
                    return;
                }
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
                        reply.requestId !== slot.snapshotPending?.id
                    )
                        return;
                    const pending = slot.snapshotPending;
                    slot.snapshotPending = undefined;
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
                if (slot === this.candidate)
                    slot.onPreview(message, response.workerMs);
                // Recovery of a retained capture must not replace the canvas.
                if (slot.pending || slot.snapshotPending) {
                    if (slot.ready) {
                        this.sendExport(slot);
                        this.sendSnapshot(slot);
                    } else
                        this.stop(
                            slot,
                            new Error("Cannot prepare retained output"),
                        );
                }
            };
            worker.postMessage(slot.capture);
        } catch (error) {
            const failure =
                error instanceof Error ? error : new Error(String(error));
            this.stop(slot, failure);
            if (slot === this.candidate) slot.onError(failure);
        }
    }
}
