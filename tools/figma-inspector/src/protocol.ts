// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

import { isWindowSize } from "./plugin/window-preferences";
import {
    isCaptureAssets,
    type AssetPreview,
    isAssetPreview,
} from "./asset-transport";
import type { TimingTrace } from "./performance/timing";
import { isTimingTrace } from "./performance/timing";
import type { Diagnostic } from "./plugin/snapshot";

export type PreviewTrigger =
    | "initial"
    | "selection-change"
    | "node-change"
    | "pin-change"
    | "density-change";

export type UiToPluginMessage =
    | { readonly type: "reset-capture-assets" }
    | { readonly type: "ui-ready"; readonly devicePixelRatio: number }
    | {
          readonly type: "pixel-density-changed";
          readonly devicePixelRatio: number;
      }
    | { readonly type: "pin-selection" }
    | { readonly type: "unpin" }
    | {
          readonly type: "resizeWindow";
          readonly width: number;
          readonly height: number;
      }
    | { readonly type: "clipboard-result"; readonly success: boolean }
    | {
          readonly type: "preview-complete";
          readonly revision: number;
          readonly trace: TimingTrace;
      };

export type PluginToUiMessage =
    | { readonly type: "preview-busy"; readonly revision: number }
    | {
          readonly type: "preview-capture";
          readonly revision: number;
          readonly captureJson: string;
          readonly captureAssetVersion?: 1 | 2;
          readonly captureAssets?: readonly (Uint8Array | number)[];
          readonly selection?: { nodeId: string; nodeName: string };
          readonly trigger?: PreviewTrigger;
          readonly trace?: TimingTrace;
      }
    | {
          readonly type: "pin-state";
          readonly pinned: boolean;
          readonly canPin: boolean;
          readonly pinnedRoot?: {
              readonly nodeId: string;
              readonly nodeName: string;
          };
      }
    | {
          readonly type: "preview-source";
          readonly revision: number;
          readonly source: string | AssetPreview;
          readonly renderSource?: string;
          readonly warnings?: readonly Diagnostic[];
          readonly snapshotJson?: string;
          readonly exportPackage?: ExportPackage;
          readonly exportError?: string;
          readonly selection?: {
              readonly nodeId: string;
              readonly nodeName: string;
          };
          readonly trigger?: PreviewTrigger;
          readonly trace?: TimingTrace;
      }
    | {
          readonly type: "preview-diagnostics";
          readonly revision: number;
          readonly diagnostics: readonly Diagnostic[];
          readonly selection?: {
              readonly nodeId: string;
              readonly nodeName: string;
          };
          readonly trigger?: PreviewTrigger;
          readonly trace?: TimingTrace;
      }
    | {
          readonly type: "preview-clear";
          readonly revision: number;
          readonly trigger?: PreviewTrigger;
          readonly trace?: TimingTrace;
      }
    | {
          readonly type: "preview-finalized";
          readonly revision: number;
          readonly trace: TimingTrace;
      };

export function isUiToPluginMessage(
    value: unknown,
): value is UiToPluginMessage {
    if (typeof value !== "object" || value === null || !("type" in value)) {
        return false;
    }
    if (
        (value.type === "ui-ready" || value.type === "pixel-density-changed") &&
        "devicePixelRatio" in value &&
        typeof value.devicePixelRatio === "number" &&
        Number.isFinite(value.devicePixelRatio) &&
        value.devicePixelRatio > 0
    )
        return true;
    if (
        value.type === "pin-selection" ||
        value.type === "unpin" ||
        value.type === "reset-capture-assets"
    )
        return true;
    if (value.type === "resizeWindow" && isWindowSize(value)) return true;
    if (
        value.type === "clipboard-result" &&
        "success" in value &&
        typeof value.success === "boolean"
    ) {
        return true;
    }
    return (
        value.type === "preview-complete" &&
        "revision" in value &&
        Number.isSafeInteger(value.revision) &&
        (value.revision as number) > 0 &&
        "trace" in value &&
        isTimingTrace(value.trace)
    );
}

export function isPluginToUiMessage(
    value: unknown,
): value is PluginToUiMessage {
    if (typeof value !== "object" || value === null || !("type" in value)) {
        return false;
    }
    if (
        value.type === "pin-state" &&
        "pinned" in value &&
        typeof value.pinned === "boolean" &&
        "canPin" in value &&
        typeof value.canPin === "boolean"
    ) {
        const candidate = value as Record<string, unknown>;
        if (!value.pinned)
            return (
                !("pinnedRoot" in candidate) ||
                candidate.pinnedRoot === undefined
            );
        const root = candidate.pinnedRoot;
        if (
            typeof root !== "object" ||
            root === null ||
            !("nodeId" in root) ||
            typeof root.nodeId !== "string" ||
            !("nodeName" in root) ||
            typeof root.nodeName !== "string"
        ) {
            return false;
        }
        return true;
    }
    if (value.type === "preview-busy")
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0
        );
    if (value.type === "preview-source" || value.type === "preview-capture") {
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0 &&
            (value.type === "preview-capture"
                ? "captureJson" in value &&
                  typeof value.captureJson === "string" &&
                  (!(
                      "captureAssets" in value || "captureAssetVersion" in value
                  ) ||
                      isCaptureAssets(value))
                : "source" in value &&
                  (typeof value.source === "string" ||
                      isAssetPreview(value.source))) &&
            (!("renderSource" in value) ||
                value.renderSource === undefined ||
                typeof value.renderSource === "string") &&
            (!("exportPackage" in value) ||
                value.exportPackage === undefined ||
                isExportPackage(value.exportPackage)) &&
            (!("exportError" in value) ||
                value.exportError === undefined ||
                typeof value.exportError === "string") &&
            (!("warnings" in value) ||
                (Array.isArray(value.warnings) &&
                    value.warnings.every(isDiagnostic))) &&
            (!("trace" in value) ||
                value.trace === undefined ||
                isTimingTrace(value.trace))
        );
    }
    if (value.type === "preview-diagnostics") {
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0 &&
            "diagnostics" in value &&
            Array.isArray(value.diagnostics) &&
            value.diagnostics.every(isDiagnostic) &&
            (!("trace" in value) ||
                value.trace === undefined ||
                isTimingTrace(value.trace))
        );
    }
    if (value.type === "preview-clear") {
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0 &&
            (!("trace" in value) ||
                value.trace === undefined ||
                isTimingTrace(value.trace))
        );
    }
    if (value.type === "preview-finalized") {
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0 &&
            "trace" in value &&
            isTimingTrace(value.trace)
        );
    }
    return false;
}

function isDiagnostic(value: unknown): value is Diagnostic {
    if (typeof value !== "object" || value === null) return false;
    const candidate = value as Record<string, unknown>;
    return (
        (candidate.severity === "error" || candidate.severity === "warning") &&
        typeof candidate.code === "string" &&
        typeof candidate.message === "string" &&
        (candidate.nodeId === undefined ||
            typeof candidate.nodeId === "string") &&
        (candidate.nodeName === undefined ||
            typeof candidate.nodeName === "string") &&
        ["nodePath", "originalValue", "fallbackAction"].every(
            (key) =>
                candidate[key] === undefined ||
                typeof candidate[key] === "string",
        ) &&
        (candidate.category === undefined ||
            ["approximation", "image", "omission", "geometry"].includes(
                String(candidate.category),
            )) &&
        (candidate.propertyPath === undefined ||
            typeof candidate.propertyPath === "string")
    );
}

export type ExportRequest = {
    kind: "export-request";
    revision: number;
    requestId: number;
};

export type ExportReply = {
    kind: "export-result";
    revision: number;
    requestId: number;
    workerMs: number;
    exportPackage?: ExportPackage;
    exportError?: string;
};
export type SnapshotRequest = {
    kind: "snapshot-request";
    revision: number;
    requestId: number;
};
export type SnapshotReply = {
    kind: "snapshot-result";
    revision: number;
    requestId: number;
    workerMs: number;
    snapshotJson?: string;
    snapshotError?: string;
};

export function isExportRequest(value: unknown): value is ExportRequest {
    if (!value || typeof value !== "object") return false;
    const candidate = value as Record<string, unknown>;
    return (
        candidate.kind === "export-request" &&
        Number.isSafeInteger(candidate.revision) &&
        (candidate.revision as number) > 0 &&
        Number.isSafeInteger(candidate.requestId) &&
        (candidate.requestId as number) > 0
    );
}

export function isSnapshotRequest(value: unknown): value is SnapshotRequest {
    if (!value || typeof value !== "object") return false;
    const v = value as Record<string, unknown>;
    return (
        v.kind === "snapshot-request" &&
        isExportRequest({ ...v, kind: "export-request" })
    );
}
export function isSnapshotReply(value: unknown): value is SnapshotReply {
    if (!value || typeof value !== "object") return false;
    const v = value as Record<string, unknown>;
    return (
        isSnapshotRequest({ ...v, kind: "snapshot-request" }) &&
        v.kind === "snapshot-result" &&
        typeof v.workerMs === "number" &&
        Number.isFinite(v.workerMs) &&
        v.workerMs >= 0 &&
        ((typeof v.snapshotJson === "string" &&
            v.snapshotError === undefined) ||
            (typeof v.snapshotError === "string" &&
                v.snapshotJson === undefined))
    );
}

export function isExportReply(value: unknown): value is ExportReply {
    if (!value || typeof value !== "object") return false;
    const candidate = value as Record<string, unknown>;
    const hasPackage = candidate.exportPackage !== undefined;
    const hasError = typeof candidate.exportError === "string";
    return (
        candidate.kind === "export-result" &&
        Number.isSafeInteger(candidate.revision) &&
        (candidate.revision as number) > 0 &&
        Number.isSafeInteger(candidate.requestId) &&
        (candidate.requestId as number) > 0 &&
        typeof candidate.workerMs === "number" &&
        Number.isFinite(candidate.workerMs) &&
        (candidate.workerMs as number) >= 0 &&
        hasPackage !== hasError &&
        (!hasPackage || candidate.exportError === undefined) &&
        (!hasPackage || isExportPackage(candidate.exportPackage))
    );
}

/** Structured, self-contained export passed from the conversion worker to UI. */
export type ExportFile = {
    path: string;
    data: string;
    encoding: "utf8" | "base64";
};
export type ExportPackage = { source: string; files: ExportFile[] };

export function isExportPackage(value: unknown): value is ExportPackage {
    if (!value || typeof value !== "object") return false;
    const result = value as ExportPackage;
    if (typeof result.source !== "string" || !Array.isArray(result.files))
        return false;
    const paths = new Set<string>();
    return result.files.every((file) => {
        if (
            !file ||
            typeof file !== "object" ||
            typeof file.path !== "string" ||
            !/^(?:assets\/[a-z0-9-]+\.(?:png|jpg|gif|svg)|fonts\/README\.txt|README\.txt)$/.test(
                file.path,
            ) ||
            paths.has(file.path) ||
            typeof file.data !== "string" ||
            !["utf8", "base64"].includes(file.encoding)
        )
            return false;
        paths.add(file.path);
        return (
            file.encoding !== "base64" ||
            /^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(
                file.data,
            )
        );
    });
}
