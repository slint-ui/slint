// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#ifndef NODE_SLINT_EMBED_H
#define NODE_SLINT_EMBED_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Entry point called from Rust main.
//
// Initializes Node.js, runs `bootstrap_js` (which is expected to load the
// user script via dynamic import after patching `Module._extensions['.node']`
// to redirect slint-ui's NAPI module to the in-process one), drains the
// libuv loop, and tears Node down.
//
// `uv_loop_out` receives a pointer to libuv's loop so Rust can register
// the winit CustomApplicationHandler before the user script runs.
//
// `before_user_script` is called after Node is initialized and the V8 context
// is entered, but before `bootstrap_js` is executed.  Use it to register
// libuv-tied resources (e.g., the winit handler) that need to outlive the
// bootstrap.
//
// Returns the exit code Node would normally exit with.
int node_slint_run(int argc,
                   char **argv,
                   const char *bootstrap_js,
                   void (*before_user_script)(int64_t uv_loop_ptr, void *userdata),
                   void *userdata);

#ifdef __cplusplus
}
#endif

#endif
