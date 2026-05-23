// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#include "embed.h"

#include <node.h>
#include <uv.h>

#include <cstdint>
#include <cstdio>
#include <memory>
#include <string>
#include <vector>

extern "C" int node_slint_run(int argc,
                              char **argv,
                              NodeSlintBody body,
                              void *userdata)
{
    argv = uv_setup_args(argc, argv);
    std::vector<std::string> args(argv, argv + argc);

    auto init = node::InitializeOncePerProcess(args);
    if (init->early_return()) {
        return init->exit_code();
    }

    std::vector<std::string> errors;
    std::unique_ptr<node::CommonEnvironmentSetup> setup =
            node::CommonEnvironmentSetup::Create(
                    init->platform(), &errors, init->args(), init->exec_args());
    if (!setup) {
        for (const auto &e : errors)
            std::fprintf(stderr, "node-slint: env error: %s\n", e.c_str());
        return 1;
    }

    v8::Isolate *isolate = setup->isolate();
    node::Environment *env = setup->env();
    uv_loop_t *uv_loop = setup->event_loop();

    {
        v8::Locker locker(isolate);
        v8::Isolate::Scope isolate_scope(isolate);
        v8::HandleScope handle_scope(isolate);
        v8::Context::Scope context_scope(setup->context());

        // Hand control to Rust, which registers the winit handler and
        // calls slint::run_event_loop().  Returns when slint exits;
        // libuv has been drained by the handler's about_to_wait logic.
        body(static_cast<void *>(uv_loop), static_cast<void *>(env), userdata);

        node::Stop(env);
    }

    setup.reset();
    node::TearDownOncePerProcess();
    return 0;
}

extern "C" void node_slint_load_environment(void *node_env, const char *script)
{
    auto *env = static_cast<node::Environment *>(node_env);
    // The outer body() call entered a HandleScope before invoking us;
    // we open a nested one so handles created by LoadEnvironment get
    // released when this function returns.
    v8::HandleScope handle_scope(v8::Isolate::GetCurrent());
    v8::MaybeLocal<v8::Value> result = node::LoadEnvironment(env, script);
    if (result.IsEmpty()) {
        std::fprintf(stderr, "node-slint: LoadEnvironment failed\n");
    }
}

extern "C" void node_slint_perform_microtask_checkpoint(void)
{
    v8::Isolate::GetCurrent()->PerformMicrotaskCheckpoint();
}
