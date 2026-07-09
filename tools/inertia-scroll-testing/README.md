# Inertia Scroll System Testing

This project uses Slint's Python system testing package to drive the inertia scroll probe out of process.
It launches the compiled probe with `SLINT_TEST_SERVER`, waits for the first window, and verifies that debug metadata exposes the scroll scene.

## Access Token

Keep the Slint testing token out of the repository.
For local runs, provide the private package index through environment variables:

```sh
export UV_INDEX="slint-private=https://testing.slint.dev/simple/"
export UV_INDEX_SLINT_PRIVATE_USERNAME="__token__"
export UV_INDEX_SLINT_PRIVATE_PASSWORD="<TOKEN>"
```

Then run:

```sh
scripts/run-inertia-scroll-system-test.sh
```

The script builds `inertia-scroll-probe` with `SLINT_EMIT_DEBUG_INFO=1` and `slint/system-testing`, then runs the Python smoke test.
Set `SLINT_BACKEND` before running the script to force a specific backend.

## CI

Use the same three environment variables in CI, sourcing `UV_INDEX_SLINT_PRIVATE_PASSWORD` from a secret.
Do not commit a tokenized `https://testing.slint.dev/t/<TOKEN>/` URL, because `uv.lock` can retain the resolved index URL.
