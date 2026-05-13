// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

/**
 * Metadata about a component property.
 */
export interface PropertyInfo {
    name: string;
    valueType: number;
}

/**
 * A diagnostic message emitted during compilation.
 */
export interface Diagnostic {
    level: number;
    message: string;
    lineNumber: number;
    columnNumber: number;
    fileName?: string;
}

/** Severity levels for diagnostics. */
export enum DiagnosticLevel {
    Error = 0,
    Warning = 1,
}

/**
 * Interface that a component definition must implement.
 * On the Node.js side, napi's ComponentDefinition already conforms to this.
 * On the WASM side, a wasm-bindgen wrapper must implement it.
 */
export interface DefinitionLike {
    create(): InstanceLike;
    readonly properties: PropertyInfo[];
    readonly callbacks: string[];
    readonly functions: string[];
    readonly globals: string[];
    globalProperties(name: string): PropertyInfo[] | null;
    globalCallbacks(name: string): string[] | null;
    globalFunctions(name: string): string[] | null;
    readonly name: string;
    readonly isWindow: boolean;
}

/**
 * Interface that a component instance must implement.
 * On the Node.js side, napi's ComponentInstance already conforms to this.
 */
export interface InstanceLike {
    definition(): DefinitionLike;
    getProperty(name: string): unknown;
    setProperty(name: string, value: unknown): void;
    setCallback(name: string, fn: Function): void;
    invoke(name: string, args: unknown[]): unknown;
    getGlobalProperty(globalName: string, name: string): unknown;
    setGlobalProperty(
        globalName: string,
        name: string,
        value: unknown,
    ): void;
    setGlobalCallback(
        globalName: string,
        name: string,
        fn: Function,
    ): void;
    invokeGlobal(
        globalName: string,
        name: string,
        args: unknown[],
    ): unknown;
    window(): unknown;
}

/**
 * Opaque handle to a native model notification object.
 * Each backend provides its own concrete type.
 */
export type ModelNotifyHandle = unknown;

/**
 * Backend interface for model change notifications.
 */
export interface ModelBackend {
    createModelNotify(): ModelNotifyHandle;
    notifyRowDataChanged(handle: ModelNotifyHandle, row: number): void;
    notifyRowAdded(
        handle: ModelNotifyHandle,
        row: number,
        count: number,
    ): void;
    notifyRowRemoved(
        handle: ModelNotifyHandle,
        row: number,
        count: number,
    ): void;
    notifyReset(handle: ModelNotifyHandle): void;
}
