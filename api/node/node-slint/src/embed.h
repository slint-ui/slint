// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#ifndef NODE_SLINT_EMBED_H
#define NODE_SLINT_EMBED_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Initialize Node.js, enter the V8 context, and call `body`.
//
// `body` runs with the V8 isolate locked and a context scope active, so
// it may freely call NAPI / V8 functions.  It receives the libuv loop
// (`uv_loop_t *`) and the `node::Environment *` as opaque pointers —
// pass these on to `node_slint_load_environment` when ready.  `body` is
// expected to run the slint event loop (which blocks until exit) and
// return when the loop has terminated.
//
// After `body` returns, Node is stopped and torn down.
typedef void (*NodeSlintBody)(void *uv_loop, void *node_env, void *userdata);

int node_slint_run(int argc, char **argv, NodeSlintBody body, void *userdata);

// Execute `script` as the Node main module.  Returns once the
// synchronous portion finishes; async work (dynamic imports, timers, …)
// is queued in libuv and processed by subsequent `uv_run` calls.
//
// Must be called from inside `body` (V8 context active).
void node_slint_load_environment(void *node_env, const char *script);

// Drain V8's microtask queue.  Needed after firing JS callbacks from
// Rust when no further libuv callback will run before we exit —
// otherwise `await` continuations queued by Promise resolution stay
// stuck in the microtask queue.
void node_slint_perform_microtask_checkpoint(void);

#ifdef __cplusplus
}
#endif

#endif
