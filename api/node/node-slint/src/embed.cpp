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
                              const char *bootstrap_js,
                              void (*before_user_script)(int64_t, void *),
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
    int exit_code = 0;

    {
        v8::Locker locker(isolate);
        v8::Isolate::Scope isolate_scope(isolate);
        v8::HandleScope handle_scope(isolate);
        v8::Context::Scope context_scope(setup->context());

        // Let Rust register backend integrations (winit CustomApplicationHandler,
        // etc.) before the user script runs.
        if (before_user_script) {
            before_user_script(reinterpret_cast<int64_t>(uv_loop), userdata);
        }

        v8::MaybeLocal<v8::Value> result = node::LoadEnvironment(env, bootstrap_js);
        if (result.IsEmpty()) {
            std::fprintf(stderr, "node-slint: failed to load bootstrap\n");
            exit_code = 1;
        }

        // Drain libuv (timers, microtasks, any code after `await runEventLoop()`).
        if (exit_code == 0) {
            exit_code = node::SpinEventLoop(env).FromMaybe(1);
        } else {
            node::SpinEventLoop(env).FromMaybe(1);
        }
        node::Stop(env);
    }

    setup.reset();
    node::TearDownOncePerProcess();
    return exit_code;
}
