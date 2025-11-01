// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export interface EventTS {
    updatePropertiesCallback: { title: string; slintSnippet: string | null };

    copyToClipboard: {
        result: boolean;
    };
    exportAll: {
        result: boolean;
    };
    exportedFiles: {
        zipFilename: string;
        files: Array<{
            name: string;
            content: string;
        }>;
    };
    monitorVariableChanges: {
        enabled: boolean;
    };
    selectionChangedInFigma: Record<string, never>;

    checkVariableChanges: Record<string, never>;
    generateSnippetRequest: { useVariables: boolean };
    nodeChanged;
    exportToFiles: { exportAsSingleFile: boolean };
    getTestData: Record<string, never>;
    createSlintExport: Record<string, never>;
    saveTextFile: {
        filename: string;
        content: string;
    };

    // Resize-related messages
    resizeWindow: { width: number; height: number };
}
