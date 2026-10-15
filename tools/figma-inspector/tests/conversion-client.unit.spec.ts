// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test, vi } from "vitest";
import {
    ConversionClient,
    type ConversionWorker,
} from "../src/ui/conversion-client";
import { TimingTraceBuilder } from "../src/performance/timing";
import type { CaptureMessage } from "../src/preview/convert-capture";
import type { ExportPackage } from "../src/protocol";
class WorkerStub implements ConversionWorker {
    onmessage: ConversionWorker["onmessage"] = null;
    onerror: ConversionWorker["onerror"] = null;
    onmessageerror: ConversionWorker["onmessageerror"] = null;
    messages: unknown[] = [];
    terminated = false;
    postMessage(value: unknown) {
        this.messages.push(value);
    }
    terminate() {
        this.terminated = true;
    }
    reply(data: unknown) {
        this.onmessage?.({ data } as MessageEvent);
    }
    ready(revision: number) {
        this.reply({
            revision,
            workerMs: 1,
            message: {
                type: "preview-source",
                revision,
                source: "export component Preview inherits Window {}",
                exportPackage: pkg,
            },
        });
    }
}
const pkg: ExportPackage = {
    source: "export component Native inherits Window {}",
    files: [],
};
const capture = (revision: number): CaptureMessage => ({
    type: "preview-capture",
    revision,
    captureJson: "{}",
});
function setup() {
    const workers: WorkerStub[] = [];
    const preview = vi.fn(),
        error = vi.fn();
    const client = new ConversionClient(() => {
        const w = new WorkerStub();
        workers.push(w);
        return w;
    });
    return {
        workers,
        preview,
        error,
        client,
        submit: (r: number) => client.capture(capture(r), preview, error),
    };
}

test("obsolete candidate is terminated and its late reply cannot replace newer work", () => {
    const s = setup();
    s.submit(1);
    s.submit(2);
    expect(s.workers[0].terminated).toBe(true);
    s.workers[0].ready(1);
    expect(s.preview).not.toHaveBeenCalled();
    s.workers[1].ready(2);
    expect(s.preview).toHaveBeenCalledOnce();
});

test("snapshot JSON is lazy, coalesced, cached and rejected when its capture retires", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    expect(s.workers[0].messages).toHaveLength(1);
    const one = s.client.snapshot(1),
        two = s.client.snapshot(1);
    expect(one).toBe(two);
    const message = s.workers[0].messages.at(-1) as { requestId: number };
    expect(message).toMatchObject({ kind: "snapshot-request", revision: 1 });
    s.workers[0].reply({
        kind: "snapshot-result",
        revision: 1,
        requestId: message.requestId,
        workerMs: 2,
        snapshotJson: '{"nodes":[]}',
    });
    expect(await one).toBe('{"nodes":[]}');
    expect(await s.client.snapshot(1)).toBe('{"nodes":[]}');
    expect(s.workers[0].messages).toHaveLength(2);
    s.submit(2);
    s.workers[1].ready(2);
    s.client.present(2);
    const pending = s.client.snapshot(2),
        rejected = expect(pending).rejects.toThrow("Capture retired");
    s.client.clear();
    await rejected;
});

test("malformed snapshots reject pending work and recover only the current capture", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    const pending = s.client.snapshot(1);
    const rejected = expect(pending).rejects.toThrow("Invalid snapshot result");
    s.workers[0].reply({
        kind: "snapshot-result",
        revision: 1,
        requestId: 1,
        workerMs: -1,
        snapshotJson: "{}",
    });
    await rejected;
    expect(s.workers[0].terminated).toBe(true);
    const retry = s.client.snapshot(1);
    expect(s.workers[1].messages[0]).toEqual(capture(1));
    s.workers[1].ready(1);
    expect(s.preview).toHaveBeenCalledOnce();
    const request = s.workers[1].messages.at(-1) as { requestId: number };
    s.workers[1].reply({
        kind: "snapshot-result",
        revision: 1,
        requestId: request.requestId,
        workerMs: 1,
        snapshotJson: "{}",
    });
    expect(await retry).toBe("{}");
    s.submit(2);
    s.client.fail(2);
    await expect(s.client.snapshot(1)).rejects.toThrow("unavailable");
});

test("new capture rejects pending snapshot work and ignores retired worker errors", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    const pending = s.client.snapshot(1);
    const rejected = expect(pending).rejects.toThrow("Capture retired");
    s.submit(2);
    await rejected;
    s.workers[0].onerror?.({} as ErrorEvent);
    expect(s.error).not.toHaveBeenCalled();
    s.workers[1].ready(2);
    expect(s.preview).toHaveBeenCalledTimes(2);
});

test("a protocol validator exception reports failure instead of stranding the preview", () => {
    const s = setup();
    s.submit(1);
    expect(() =>
        s.workers[0].reply({
            revision: 1,
            workerMs: 1,
            message: {
                get type() {
                    throw new RangeError("validator overflow");
                },
            },
        }),
    ).not.toThrow();
    expect(s.error).toHaveBeenCalledWith(
        expect.objectContaining({ message: "validator overflow" }),
    );
    expect(s.workers[0].terminated).toBe(true);
    s.submit(2);
    s.workers[1].ready(2);
    expect(s.preview).toHaveBeenCalledOnce();
});

test("a delayed export updates its preview without publishing it twice and ignores retired results", () => {
    const workers: WorkerStub[] = [];
    const preview = vi.fn(),
        failed = vi.fn(),
        exported = vi.fn();
    const client = new ConversionClient(() => {
        const worker = new WorkerStub();
        workers.push(worker);
        return worker;
    });
    client.capture(capture(1), preview, failed, exported);
    workers[0].ready(1);
    const reply = {
        revision: 1,
        workerMs: 2,
        message: {
            type: "preview-export",
            revision: 1,
            exportPackage: pkg,
            warnings: [],
        },
    };
    workers[0].reply(reply);
    expect(preview).toHaveBeenCalledOnce();
    expect(exported).toHaveBeenCalledWith(reply.message);
    client.capture(capture(2), preview, failed, exported);
    workers[0].reply(reply);
    expect(exported).toHaveBeenCalledOnce();
    workers[1].ready(2);
    workers[1].reply({
        revision: 2,
        workerMs: 2,
        message: {
            type: "preview-export",
            revision: 2,
            exportPackage: null,
            warnings: [],
        },
    });
    expect(failed).not.toHaveBeenCalled();
    expect(exported).toHaveBeenLastCalledWith(
        expect.objectContaining({
            revision: 2,
            warnings: [expect.objectContaining({ category: "export" })],
        }),
    );
});

test("cached conversion rebinds revision without another worker or capture transfer", async () => {
    const s = setup();
    const exported = vi.fn();
    s.client.capture(
        { ...capture(1), captureId: "same" },
        s.preview,
        s.error,
        exported,
    );
    s.workers[0].ready(1);
    s.workers[0].reply({
        revision: 1,
        workerMs: 1,
        message: {
            type: "preview-export",
            revision: 1,
            warnings: [],
            exportPackage: pkg,
        },
    });
    s.client.present(1);
    s.client.clear();
    const trace = new TimingTraceBuilder(3, "selection-change").toTrace();
    expect(s.client.reuse("same", 3, trace, s.preview, s.error, exported)).toBe(
        true,
    );
    expect(s.workers).toHaveLength(1);
    expect(s.workers[0].terminated).toBe(false);
    expect(s.preview).toHaveBeenLastCalledWith(
        expect.objectContaining({ revision: 3, trace }),
        0,
    );
    expect(exported).toHaveBeenLastCalledWith(
        expect.objectContaining({ revision: 3, exportPackage: pkg }),
    );
    s.client.present(3);
    const snapshot = s.client.snapshot(3);
    const request = s.workers[0].messages.at(-1) as { requestId: number };
    s.workers[0].reply({
        kind: "snapshot-result",
        revision: 1,
        requestId: request.requestId,
        workerMs: 0,
        snapshotJson: "{}",
    });
    expect(await snapshot).toBe("{}");
    s.client.dispose();
    expect(s.workers[0].terminated).toBe(true);
});

test("conversion eviction and incomplete results request fresh data rather than reuse stale work", () => {
    const s = setup();
    for (let revision = 1; revision <= 3; revision++) {
        s.client.capture(
            { ...capture(revision), captureId: String(revision) },
            s.preview,
            s.error,
        );
        s.workers[revision - 1].ready(revision);
        s.workers[revision - 1].reply({
            revision,
            workerMs: 0,
            message: {
                type: "preview-export",
                revision,
                warnings: [],
                exportPackage: pkg,
            },
        });
        s.client.clear();
    }
    expect(s.workers[0].terminated).toBe(true);
    expect(
        s.client.reuse(
            "1",
            4,
            new TimingTraceBuilder(4, "selection-change").toTrace(),
            s.preview,
            s.error,
        ),
    ).toBe(false);
    s.submit(4);
    s.workers[3].ready(4);
    s.client.clear();
    expect(s.workers[3].terminated).toBe(true);
    s.client.dispose();
});

test("bulk preparation overlaps capture and the same worker performs conversion", () => {
    const s = setup();
    s.client.prepareBulk({
        type: "preview-bulk",
        revision: 1,
        document: { id: "root", type: "FRAME" },
    });
    expect(s.workers).toHaveLength(1);
    s.submit(1);
    expect(s.workers).toHaveLength(1);
    expect(
        s.workers[0].messages.map(
            (message) => (message as { type: string }).type,
        ),
    ).toEqual(["preview-bulk", "preview-capture"]);
    s.workers[0].ready(1);
    expect(s.preview).toHaveBeenCalledOnce();
});

test("obsolete bulk work is terminated and worker errors still permit self-contained capture", () => {
    const s = setup();
    const document = { id: "root", type: "FRAME" };
    s.client.prepareBulk({ type: "preview-bulk", revision: 1, document });
    s.client.prepareBulk({ type: "preview-bulk", revision: 2, document });
    expect(s.workers[0].terminated).toBe(true);
    s.workers[1].onerror?.({} as ErrorEvent);
    expect(s.workers[1].terminated).toBe(true);
    s.submit(2);
    expect(s.workers).toHaveLength(3);
    s.workers[2].ready(2);
    expect(s.preview).toHaveBeenCalledOnce();
    s.client.prepareBulk({ type: "preview-bulk", revision: 3, document });
    s.client.dispose();
    expect(s.workers[3].terminated).toBe(true);
});

test("activate a large retained result before parking the outgoing selection", () => {
    const s = setup();
    const large = " ".repeat(18 * 1024 * 1024);
    const complete = (revision: number, id: string) => {
        s.client.capture(
            { ...capture(revision), captureId: id, captureJson: large },
            s.preview,
            s.error,
        );
        const worker = s.workers.at(-1)!;
        worker.ready(revision);
        worker.reply({
            revision,
            workerMs: 0,
            message: {
                type: "preview-export",
                revision,
                warnings: [],
                exportPackage: pkg,
            },
        });
        s.client.present(revision);
    };
    complete(1, "A");
    s.client.reserve();
    complete(2, "B");
    s.client.reserve();
    expect(
        s.client.reuse(
            "A",
            3,
            new TimingTraceBuilder(3, "selection-change").toTrace(),
            s.preview,
            s.error,
        ),
    ).toBe(true);
    s.client.reserve();
    expect(
        s.client.reuse(
            "B",
            4,
            new TimingTraceBuilder(4, "selection-change").toTrace(),
            s.preview,
            s.error,
        ),
    ).toBe(true);
    expect(s.workers).toHaveLength(2);
    expect(s.workers.every((worker) => !worker.terminated)).toBe(true);
    s.client.dispose();
});
