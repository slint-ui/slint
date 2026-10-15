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
    | {
          readonly type: "capture-reuse-miss";
          readonly revision: number;
          readonly captureId: string;
      }
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
    | {
          readonly type: "preview-bulk";
          readonly revision: number;
          readonly document: Record<string, unknown>;
      }
    | {
          readonly type: "preview-reuse";
          readonly revision: number;
          readonly captureId: string;
          readonly selection?: { nodeId: string; nodeName: string };
          readonly trigger?: PreviewTrigger;
          readonly trace?: TimingTrace;
      }
    | {
          readonly type: "preview-export";
          readonly revision: number;
          readonly exportPackage?: ExportPackage;
          readonly warnings: readonly Diagnostic[];
      }
    | {
          readonly type: "preview-busy";
          readonly revision: number;
      }
    | {
          readonly type: "preview-capture";
          readonly revision: number;
          readonly captureJson: string;
          readonly captureId?: string;
          readonly captureAssetVersion?: 2;
          readonly captureAssets?: readonly (Uint8Array | number)[];
          readonly warnings?: readonly Diagnostic[];
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
          readonly warnings?: readonly Diagnostic[];
          readonly exportPackage?: ExportPackage;
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
    if (value.type === "capture-reuse-miss")
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0 &&
            "captureId" in value &&
            typeof value.captureId === "string" &&
            value.captureId.length > 0 &&
            value.captureId.length <= 128
        );
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
    if (value.type === "preview-bulk")
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0 &&
            "document" in value &&
            !!value.document &&
            typeof value.document === "object" &&
            !Array.isArray(value.document) &&
            "id" in value.document &&
            typeof value.document.id === "string" &&
            "type" in value.document &&
            typeof value.document.type === "string"
        );
    if (value.type === "preview-busy")
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0
        );
    if (value.type === "preview-export")
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0 &&
            (!("exportPackage" in value) ||
                value.exportPackage === undefined ||
                isExportPackage(value.exportPackage)) &&
            "warnings" in value &&
            Array.isArray(value.warnings) &&
            value.warnings.every(isDiagnostic)
        );
    if (value.type === "preview-reuse")
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0 &&
            "captureId" in value &&
            typeof value.captureId === "string" &&
            value.captureId.length > 0 &&
            value.captureId.length <= 128 &&
            (!("trace" in value) ||
                value.trace === undefined ||
                isTimingTrace(value.trace))
        );
    if (value.type === "preview-source" || value.type === "preview-capture") {
        return (
            "revision" in value &&
            Number.isSafeInteger(value.revision) &&
            (value.revision as number) > 0 &&
            (value.type === "preview-capture"
                ? "captureJson" in value &&
                  typeof value.captureJson === "string" &&
                  (!("captureId" in value) ||
                      value.captureId === undefined ||
                      (typeof value.captureId === "string" &&
                          value.captureId.length > 0 &&
                          value.captureId.length <= 128)) &&
                  (!(
                      "captureAssets" in value || "captureAssetVersion" in value
                  ) ||
                      isCaptureAssets(value))
                : "source" in value &&
                  (typeof value.source === "string" ||
                      isAssetPreview(value.source))) &&
            (value.type !== "preview-source" ||
                !("exportPackage" in value) ||
                value.exportPackage === undefined ||
                isExportPackage(value.exportPackage)) &&
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
            [
                "approximation",
                "image",
                "omission",
                "geometry",
                "export",
            ].includes(String(candidate.category))) &&
        (candidate.propertyPath === undefined ||
            typeof candidate.propertyPath === "string")
    );
}

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

export function isSnapshotRequest(value: unknown): value is SnapshotRequest {
    if (!value || typeof value !== "object") return false;
    const v = value as Record<string, unknown>;
    return (
        v.kind === "snapshot-request" &&
        Number.isSafeInteger(v.revision) &&
        (v.revision as number) > 0 &&
        Number.isSafeInteger(v.requestId) &&
        (v.requestId as number) > 0
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
        return file.encoding !== "base64" || isBase64(file.data);
    });
}

function isBase64(value: string): boolean {
    if (value.length % 4 !== 0) return false;
    const padding = value.endsWith("==") ? 2 : value.endsWith("=") ? 1 : 0;
    for (let index = 0; index < value.length - padding; index++) {
        const code = value.charCodeAt(index);
        if (
            !(code >= 65 && code <= 90) &&
            !(code >= 97 && code <= 122) &&
            !(code >= 48 && code <= 57) &&
            code !== 43 &&
            code !== 47
        )
            return false;
    }
    return true;
}
