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
