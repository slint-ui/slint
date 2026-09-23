// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

declare module "@interpreter/slint_wasm_interpreter_bg.wasm?url&inline" {
    const url: string;
    export default url;
}

declare module "@interpreter/slint_wasm_interpreter.js" {
    export interface WrappedInstance {
        free(): void;
        hide(): Promise<unknown>;
        show(): Promise<unknown>;
    }

    interface WrappedCompiledComponent {
        free(): void;
        create(canvasId: string): Promise<WrappedInstance>;
        create_with_existing_window(
            instance: WrappedInstance,
        ): Promise<WrappedInstance>;
    }

    interface CompilationResult {
        readonly component: WrappedCompiledComponent | undefined;
        readonly error_string: string;
        free(): void;
    }

    export function compile_from_string(
        source: string,
        baseUrl: string,
        importCallback?: ((url: string) => Promise<string>) | null,
    ): Promise<CompilationResult>;
    export function run_event_loop(): void;
    export default function initialize(input: {
        module_or_path: BufferSource | WebAssembly.Module;
    }): Promise<unknown>;
}
