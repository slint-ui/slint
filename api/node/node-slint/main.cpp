// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// node-slint: a custom Node.js runner with native Slint event loop integration.
//
// Instead of polling winit from JavaScript at 16 ms intervals (as plain `node`
// does), this binary embeds Node.js via libnode and lets winit own the event
// loop.  libuv is ticked inside winit's `about_to_wait()` callback through
// a CustomApplicationHandler registered by the NAPI addon's `initPlatform()`.
//
// Usage:  node-slint my-app.js

#include <node.h>
#include <uv.h>

#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

int main(int argc, char *argv[])
{
    if (argc < 2) {
        fprintf(stderr, "Usage: node-slint <script.js> [args...]\n");
        return 1;
    }

    argv = uv_setup_args(argc, argv);

    std::vector<std::string> args(argv, argv + argc);

    std::shared_ptr<node::InitializationResult> init =
            node::InitializeOncePerProcess(args);
    if (init->early_return()) {
        return init->exit_code();
    }

    node::MultiIsolatePlatform *platform = init->platform();
    std::vector<std::string> errors;
    std::unique_ptr<node::CommonEnvironmentSetup> setup =
            node::CommonEnvironmentSetup::Create(
                platform, &errors, init->args(), init->exec_args());
    if (!setup) {
        for (const auto &e : errors)
            fprintf(stderr, "node env error: %s\n", e.c_str());
        return 1;
    }

    v8::Isolate *isolate = setup->isolate();
    node::Environment *env = setup->env();

    // The libuv loop used by this Node.js instance.
    uv_loop_t *uv_loop = setup->event_loop();
    int exit_code = 0;

    {
        v8::Locker locker(isolate);
        v8::Isolate::Scope isolate_scope(isolate);
        v8::HandleScope handle_scope(isolate);
        v8::Context::Scope context_scope(setup->context());

        // Set up globals for the NAPI addon, initialize the Slint platform
        // with the libuv handler, then load the user script via dynamic
        // import() so both ESM and CJS work.
        std::string bootstrap =
                "globalThis.__slint_native_loop = true;\n"
                "globalThis.__slint_uv_loop_ptr = "
                + std::to_string(reinterpret_cast<std::int64_t>(uv_loop))
                + ";\n"
                  "const { createRequire } = require('module');\n"
                  "const __path = require('path');\n"
                  "const { pathToFileURL } = require('url');\n"
                  "const __script = __path.resolve(process.argv[1]);\n"
                  "const __require = createRequire(__script);\n"
                  "try {\n"
                  "  const __napi = __require('slint-ui/rust-module.cjs');\n"
                  "  __napi.initPlatform(globalThis.__slint_uv_loop_ptr);\n"
                  "} catch(e) {\n"
                  "  if (e?.code !== 'MODULE_NOT_FOUND')\n"
                  "    process.stderr.write('node-slint: initPlatform failed: ' + e + '\\n');\n"
                  "}\n"
                  "import(pathToFileURL(__script));\n";

        v8::MaybeLocal<v8::Value> result = node::LoadEnvironment(env, bootstrap.c_str());

        if (result.IsEmpty()) {
            fprintf(stderr, "node-slint: failed to load script\n");
            exit_code = 1;
        }

        // Drain remaining libuv work (timers, promises) after the user
        // script and any blocking runEventLoop() call have completed.
        exit_code = node::SpinEventLoop(env).FromMaybe(1);
        node::Stop(env);
    }

    setup.reset();
    node::TearDownOncePerProcess();

    return exit_code;
}
