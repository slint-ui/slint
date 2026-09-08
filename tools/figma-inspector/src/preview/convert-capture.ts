// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { generateExport } from "../export/generate";
import { CaptureAssetReceiver, unpackCaptureAssets } from "../asset-transport";
import {
    defaultClock,
    TimingTraceBuilder,
    type TimingPhase,
} from "../performance/timing";
import type { PluginToUiMessage } from "../protocol";
import {
    createSourceNormalizer,
    type SourceNormalizer,
} from "../plugin/normalize";
import type { FigmaSnapshot } from "../plugin/snapshot";
import type { SourceBytes, SourceCapture } from "../plugin/source";
import { packPreviewAssets } from "../asset-transport";
import { convertSnapshot } from "./converter";

export type CaptureMessage = Extract<
    PluginToUiMessage,
    { type: "preview-capture" }
>;
export type ConvertedMessage = Extract<
    PluginToUiMessage,
    { type: "preview-source" | "preview-diagnostics" | "preview-clear" }
>;

export type CaptureConversionState = {
    source?: SourceCapture<SourceBytes>;
    normalizer?: SourceNormalizer;
    snapshot?: FigmaSnapshot;
    snapshotJson?: string;
};

export function captureSnapshotJson(state: CaptureConversionState): string {
    if (!state.snapshot) throw Error("Snapshot is unavailable");
    state.snapshotJson ??= JSON.stringify(state.snapshot);
    return state.snapshotJson;
}

function parseCapture(
    request: CaptureMessage,
    assetCache: CaptureAssetReceiver,
    state: CaptureConversionState,
): SourceCapture<SourceBytes> {
    if (state.source) return state.source;
    state.source = (
        request.captureAssetVersion !== undefined
            ? unpackCaptureAssets(
                  request.captureJson,
                  assetCache.resolve(request.captureAssets ?? []),
                  "binary",
              )
            : JSON.parse(request.captureJson)
    ) as SourceCapture<SourceBytes>;
    return state.source;
}

export async function convertExport(
    request: CaptureMessage,
    assetCache = new CaptureAssetReceiver(),
    state: CaptureConversionState = {},
): Promise<import("../protocol").ExportPackage> {
    const source = parseCapture(request, assetCache, state);
    state.normalizer ??= createSourceNormalizer(source);
    const native = await state.normalizer.normalize("export");
    if (!native.ok || native.empty)
        throw Error(
            native.ok
                ? "Export contains no nodes"
                : native.diagnostics.map((d) => d.message).join("\n"),
        );
    return generateExport(native.snapshot, native.warnings);
}

// Pure captured-data pipeline. This module cannot access Figma or the DOM.
export async function convertCapture(
    request: CaptureMessage,
    assetCache = new CaptureAssetReceiver(),
    state: CaptureConversionState = {},
): Promise<ConvertedMessage> {
    const original =
        request.trace ??
        new TimingTraceBuilder(
            request.revision,
            request.trigger ?? "initial",
        ).toTrace();
    const trace = { ...original, phases: { ...original.phases } };
    const common = {
        revision: request.revision,
        trigger: request.trigger,
        selection: request.selection,
        trace,
    };
    const measure = <T>(phase: TimingPhase, operation: () => T): T => {
        const start = defaultClock.monotonicNow();
        try {
            return operation();
        } finally {
            trace.phases[phase] = defaultClock.monotonicNow() - start;
        }
    };
    try {
        const source = measure("captureParse", () =>
            parseCapture(request, assetCache, state),
        );
        const start = defaultClock.monotonicNow();
        state.normalizer ??= createSourceNormalizer(source);
        const normalized = await state.normalizer.normalize();
        trace.phases.normalization = defaultClock.monotonicNow() - start;
        trace.captureMetrics = {
            ...normalized.captureMetrics,
            ...original.captureMetrics,
            durationMs:
                original.captureMetrics?.durationMs ??
                normalized.captureMetrics.durationMs,
        };
        if (!normalized.ok)
            return {
                ...common,
                type: "preview-diagnostics",
                diagnostics: normalized.diagnostics,
                trace: { ...trace, outcome: "capture-error" },
            };
        if (normalized.empty) return { ...common, type: "preview-clear" };
        state.snapshot = normalized.snapshot;
        const [converted, render] = measure("slintConversion", () => [
            convertSnapshot(normalized.snapshot),
            convertSnapshot(normalized.snapshot, { specialize: true }),
        ]);
        if (!converted.ok)
            return {
                ...common,
                type: "preview-diagnostics",
                diagnostics: converted.diagnostics,
                trace: { ...trace, outcome: "conversion-error" },
            };
        if (!render.ok)
            return {
                ...common,
                type: "preview-diagnostics",
                diagnostics: render.diagnostics,
                trace: { ...trace, outcome: "conversion-error" },
            };
        const packed = measure("assetPacking", () =>
            packPreviewAssets(converted.source, "", render.source),
        );
        return {
            ...common,
            type: "preview-source",
            source: packed.assets.length ? packed : converted.source,
            ...(packed.assets.length ? {} : { renderSource: render.source }),
            warnings: [...normalized.warnings, ...converted.warnings],
        };
    } catch (error) {
        return {
            ...common,
            type: "preview-diagnostics",
            trace: { ...trace, outcome: "conversion-error" },
            diagnostics: [
                {
                    severity: "error",
                    code: "CAPTURE_CONVERSION_ERROR",
                    message:
                        error instanceof Error ? error.message : String(error),
                },
            ],
        };
    }
}
