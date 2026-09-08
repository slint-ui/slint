// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { expect, test, vi } from "vitest";
import {
    ConversionClient,
    type ConversionWorker,
} from "../src/ui/conversion-client";
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
            },
        });
    }
    complete(
        revision: number,
        result: { exportPackage: ExportPackage } | { exportError: string },
    ) {
        const message = this.messages.at(-1) as { requestId: number };
        this.reply({
            kind: "export-result",
            revision,
            requestId: message.requestId,
            workerMs: 4,
            ...result,
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

test("preview alone never requests native export; demand coalesces and caches", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    expect(s.workers[0].messages).toHaveLength(1);
    const one = s.client.export(1),
        two = s.client.export(1);
    expect(one).toBe(two);
    expect(s.workers[0].messages).toHaveLength(2);
    s.workers[0].complete(1, { exportPackage: pkg });
    expect(await one).toEqual(pkg);
    expect(await s.client.export(1)).toEqual(pkg);
    expect(s.workers[0].messages).toHaveLength(2);
});
test("native failure rejects all coalesced callers and retries", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    const pending = s.client.export(1);
    const rejection = expect(pending).rejects.toThrow("native failed");
    s.workers[0].complete(1, { exportError: "native failed" });
    await rejection;
    const retry = s.client.export(1);
    s.workers[0].complete(1, { exportPackage: pkg });
    expect(await retry).toEqual(pkg);
});
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
test("multiple failed new revisions retain the previously ungenerated export", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    for (const rev of [2, 3, 4]) {
        s.submit(rev);
        s.workers.at(-1)?.ready(rev);
        s.client.fail(rev);
    }
    expect(s.client.has(1)).toBe(true);
    expect(s.workers[0].terminated).toBe(false);
    const pending = s.client.export(1);
    s.workers[0].complete(1, { exportPackage: pkg });
    expect(await pending).toEqual(pkg);
});
test("new capture interrupts old export; retained input can recover without presenting again", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    const old = s.client.export(1);
    const rejected = expect(old).rejects.toThrow("Selection changed");
    s.submit(2);
    await rejected;
    expect(s.workers[0].terminated).toBe(true);
    s.client.fail(2);
    const retry = s.client.export(1);
    expect(s.workers[2].messages[0]).toEqual(capture(1));
    s.workers[2].ready(1);
    expect(s.preview).toHaveBeenCalledOnce();
    s.workers[2].complete(1, { exportPackage: pkg });
    expect(await retry).toEqual(pkg);
});
test("promotion retires previous context and clear rejects pending work", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    s.submit(2);
    s.workers[1].ready(2);
    s.client.present(2);
    expect(s.workers[0].terminated).toBe(true);
    expect(s.client.has(1)).toBe(false);
    const pending = s.client.export(2);
    const rejected = expect(pending).rejects.toThrow("Capture retired");
    s.client.clear();
    await rejected;
    expect(s.client.has(2)).toBe(false);
    expect(s.workers[1].terminated).toBe(true);
});
test("malformed export replies reject requests and allow recovery", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    const pending = s.client.export(1);
    const rejected = expect(pending).rejects.toThrow("Invalid export result");
    s.workers[0].reply({
        kind: "export-result",
        revision: 1,
        requestId: 1,
        workerMs: -1,
        exportPackage: pkg,
    });
    await rejected;
    const retry = s.client.export(1);
    s.workers[1].ready(1);
    s.workers[1].complete(1, { exportPackage: pkg });
    expect(await retry).toEqual(pkg);
});
test("presented worker errors reject export promises rather than leaving controls busy", async () => {
    const s = setup();
    s.submit(1);
    s.workers[0].ready(1);
    s.client.present(1);
    const pending = s.client.export(1);
    const rejected = expect(pending).rejects.toThrow(
        "Conversion worker failed",
    );
    s.workers[0].onerror?.({} as ErrorEvent);
    await rejected;
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
