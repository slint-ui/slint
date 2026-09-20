// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { describe, expect, test } from "vitest";

import { isPluginToUiMessage, isUiToPluginMessage } from "../src/protocol";
import { isSnapshotRequest, isSnapshotReply } from "../src/protocol";

describe("protocol-validation", () => {
    test("preview busy accepts only an optional string status", () => {
        expect(
            isPluginToUiMessage({
                type: "preview-busy",
                revision: 1,
                message: "Preparing a simplified preview",
            }),
        ).toBe(true);
        expect(
            isPluginToUiMessage({
                type: "preview-busy",
                revision: 1,
                message: 20,
            }),
        ).toBe(false);
    });

    test("diagnostics reject malformed elements and revisions", () => {
        for (const revision of [0, -1, 1.5, Infinity, NaN])
            expect(
                isPluginToUiMessage({
                    type: "preview-diagnostics",
                    revision,
                    diagnostics: [],
                }),
            ).toBe(false);
        for (const diagnostic of [null, {}, { message: "bad" }])
            expect(
                isPluginToUiMessage({
                    type: "preview-diagnostics",
                    revision: 1,
                    diagnostics: [diagnostic],
                }),
            ).toBe(false);
        expect(
            isPluginToUiMessage({
                type: "preview-diagnostics",
                revision: 1,
                diagnostics: [
                    { severity: "error", code: "TEST", message: "Valid error" },
                ],
            }),
        ).toBe(true);
    });
});

describe("conversion-protocol", () => {
    test("validates positive safe revision and request ids", () => {
        expect(
            isSnapshotRequest({
                kind: "snapshot-request",
                revision: 1,
                requestId: 2,
            }),
        ).toBe(true);
        expect(
            isSnapshotRequest({
                kind: "snapshot-request",
                revision: 0,
                requestId: 2,
            }),
        ).toBe(false);
        expect(
            isSnapshotRequest({
                kind: "snapshot-request",
                revision: 1,
                requestId: 0,
            }),
        ).toBe(false);
        expect(
            isSnapshotRequest({
                kind: "snapshot-request",
                revision: 1.5,
                requestId: 2,
            }),
        ).toBe(false);
        expect(
            isSnapshotRequest({
                kind: "snapshot-request",
                revision: Number.MAX_SAFE_INTEGER + 1,
                requestId: 2,
            }),
        ).toBe(false);
    });

    test("snapshot protocol rejects invalid ids, timing and ambiguous results", () => {
        const request = { kind: "snapshot-request", revision: 1, requestId: 2 };
        expect(isSnapshotRequest(request)).toBe(true);
        expect(isSnapshotRequest({ ...request, requestId: -1 })).toBe(false);
        const reply = {
            ...request,
            kind: "snapshot-result",
            workerMs: 1,
            snapshotJson: "{}",
        };
        expect(isSnapshotReply(reply)).toBe(true);
        expect(isSnapshotReply({ ...reply, workerMs: Infinity })).toBe(false);
        expect(isSnapshotReply({ ...reply, snapshotError: "error" })).toBe(
            false,
        );
        expect(isSnapshotReply({ ...reply, snapshotJson: undefined })).toBe(
            false,
        );
    });
});

describe("milestone7", () => {
    test("accepts Inspector resize messages at or above its minimum dimensions", () => {
        expect(
            isUiToPluginMessage({
                type: "resizeWindow",
                width: 640,
                height: 720,
            }),
        ).toBe(true);
        expect(
            isUiToPluginMessage({
                type: "resizeWindow",
                width: 640.5,
                height: 720,
            }),
        ).toBe(false);
        expect(
            isUiToPluginMessage({
                type: "resizeWindow",
                width: Number.POSITIVE_INFINITY,
                height: 720,
            }),
        ).toBe(false);
        expect(
            isUiToPluginMessage({
                type: "resizeWindow",
                width: 499,
                height: 720,
            }),
        ).toBe(false);
        expect(
            isUiToPluginMessage({
                type: "resizeWindow",
                width: 640,
                height: 319,
            }),
        ).toBe(false);
    });
});

describe("milestone7", () => {
    test("accepts only finite positive device pixel ratio messages", () => {
        for (const devicePixelRatio of [1, 1.25, 2, 3]) {
            expect(
                isUiToPluginMessage({ type: "ui-ready", devicePixelRatio }),
            ).toBe(true);
            expect(
                isUiToPluginMessage({
                    type: "pixel-density-changed",
                    devicePixelRatio,
                }),
            ).toBe(true);
        }
        for (const devicePixelRatio of [
            0,
            -1,
            Number.NaN,
            Number.POSITIVE_INFINITY,
        ]) {
            expect(
                isUiToPluginMessage({
                    type: "pixel-density-changed",
                    devicePixelRatio,
                }),
            ).toBe(false);
        }
        expect(isUiToPluginMessage({ type: "ui-ready" })).toBe(false);
    });
});
