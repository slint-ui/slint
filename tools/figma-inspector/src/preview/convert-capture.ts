// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { hydrateBulkSource } from "../plugin/bulk-source";
import { generateExport } from "../export/generate";
import { unpackCaptureAssets } from "../asset-transport";
import {
    defaultClock,
    TimingTraceBuilder,
    type TimingPhase,
} from "../performance/timing";
import type { ExportPackage, PluginToUiMessage } from "../protocol";
import { createSourceNormalizer } from "../plugin/normalize";
import type { Diagnostic, FigmaSnapshot } from "../plugin/snapshot";
import type { SourceBytes, SourceCapture } from "../plugin/source";
import { packPreviewAssets } from "../asset-transport";
import { scalePreview } from "./scale-preview";
import { convertSnapshot } from "./converter";
import { exportFailure } from "./export-diagnostic";

export type CaptureMessage = Extract<
    PluginToUiMessage,
    { type: "preview-capture" }
>;
export type ConvertedMessage = Extract<
    PluginToUiMessage,
    { type: "preview-source" | "preview-diagnostics" | "preview-clear" }
>;

export type CaptureConversionState = {
    snapshot?: FigmaSnapshot;
    bulkNodes?: Map<string, Record<string, unknown>>;
    bulkDocuments?: Set<string>;
    bulkIndexMs?: number;
};

export function captureSnapshotJson(state: CaptureConversionState): string {
    if (!state.snapshot) throw Error("Snapshot is unavailable");
    return JSON.stringify(state.snapshot);
}

// Pure captured-data pipeline. This module cannot access Figma or the DOM.
export async function convertCapture(
    request: CaptureMessage,
    state: CaptureConversionState = {},
    onPreview?: (
        message: Extract<ConvertedMessage, { type: "preview-source" }>,
    ) => void,
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
        const source = measure(
            "captureParse",
            () =>
                (request.captureAssetVersion !== undefined
                    ? unpackCaptureAssets(
                          request.captureJson,
                          request.captureAssets as readonly Uint8Array[],
                      )
                    : JSON.parse(
                          request.captureJson,
                      )) as SourceCapture<SourceBytes>,
        );
        const start = defaultClock.monotonicNow();
        const hydrationStarted = defaultClock.monotonicNow();
        const normalizer = createSourceNormalizer(
            source.bulkDocuments
                ? hydrateBulkSource(
                      source,
                      state.bulkNodes,
                      state.bulkDocuments,
                  )
                : source,
        );
        trace.phases.bulkHydration =
            defaultClock.monotonicNow() - hydrationStarted;
        const normalized = await normalizer.normalize();
        trace.phases.normalization =
            defaultClock.monotonicNow() - start - trace.phases.bulkHydration;
        trace.captureMetrics = {
            ...normalized.captureMetrics,
            ...original.captureMetrics,
            ...(original.captureMetrics?.timings
                ? {
                      timings: {
                          ...original.captureMetrics.timings,
                          bulkWorkerIndexMs: state.bulkIndexMs ?? 0,
                      },
                  }
                : {}),
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
        const render = measure("slintConversion", () => {
            const converted = convertSnapshot(normalized.snapshot, {
                specialize: true,
            });
            if (!converted.ok) return converted;
            const scale = Math.min(
                1,
                2048 / Math.max(converted.width, converted.height),
            );
            return scale === 1
                ? converted
                : convertSnapshot(scalePreview(normalized.snapshot, scale), {
                      specialize: true,
                  });
        });
        if (!render.ok)
            return {
                ...common,
                type: "preview-diagnostics",
                diagnostics: render.diagnostics,
                trace: { ...trace, outcome: "conversion-error" },
            };
        const packed = measure("assetPacking", () =>
            packPreviewAssets(render.source),
        );
        onPreview?.({
            ...common,
            type: "preview-source",
            source: packed.assets.length ? packed : render.source,
            warnings: [
                ...normalized.warnings,
                ...render.warnings,
                ...(request.warnings ?? []),
            ],
            trace: { ...trace, phases: { ...trace.phases } },
        });
        let exportPackage: ExportPackage | undefined;
        let exportWarnings: readonly Diagnostic[] = [];
        try {
            const nativeStarted = defaultClock.monotonicNow();
            const native = await normalizer.normalize("export");
            trace.phases.normalization +=
                defaultClock.monotonicNow() - nativeStarted;
            if (!native.ok || native.empty) {
                exportWarnings = (
                    !native.ok
                        ? native.diagnostics
                        : [
                              {
                                  severity: "error" as const,
                                  code: "EMPTY_EXPORT",
                                  message: "Export contains no nodes",
                              },
                          ]
                ).map((diagnostic) => ({
                    ...diagnostic,
                    ...exportFailure(diagnostic.message, diagnostic.code),
                }));
            } else {
                const exportStarted = defaultClock.monotonicNow();
                try {
                    const exported = generateExport(
                        native.snapshot,
                        native.warnings,
                    );
                    exportPackage = exported.exportPackage;
                    exportWarnings = exported.warnings;
                } finally {
                    trace.phases.slintConversion =
                        (trace.phases.slintConversion ?? 0) +
                        defaultClock.monotonicNow() -
                        exportStarted;
                }
            }
        } catch (error) {
            exportWarnings = [exportFailure(error)];
        }
        return {
            ...common,
            type: "preview-source",
            source: packed.assets.length ? packed : render.source,
            ...(exportPackage ? { exportPackage } : {}),
            warnings: [
                ...new Map(
                    [
                        ...normalized.warnings,
                        ...render.warnings,
                        ...exportWarnings,
                        ...(request.warnings ?? []),
                    ].map((warning) => [JSON.stringify(warning), warning]),
                ).values(),
            ],
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
