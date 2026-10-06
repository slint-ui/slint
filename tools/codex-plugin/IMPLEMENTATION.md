# Implementation checklist

- [x] Bind each file-backed preview and inline edit to its source path, revision, and hash.
- [x] Reject rendering when saved source differs from the validated hash.
- [x] Report successful paint and failures to model context with source identity.
- [x] Resolve relative project imports, images, and fonts through bounded MCP resources.
- [x] Test the same-file color, label, and size edit cycle, including stale-source rejection.
- [x] Build a platform-specific team archive with matching runtime artifacts and installation instructions.
- [x] Run a deletion-focused audit across the plugin and apply justified deletions.
- [x] Run a simplification audit across the changed paths and apply simplifications.
- [x] Run integration, Wasm, packaging, and desktop-backend checks.
- [x] Commit and prepare the updated local plugin; leave visible host acceptance explicit.

Host acceptance after refresh remains a user check; browser rendering and backend transport are independently verified.
