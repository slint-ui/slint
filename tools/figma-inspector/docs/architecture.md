# Architecture

## Capture and conversion

`src/plugin/capture.ts` is the sole live Figma node API boundary. It captures
selection geometry, appearance, required component definitions, variables and
assets into versioned source JSON. Production caching reuses text/image exports;
a shared scheduler bounds in-flight host exports across selection revisions.
Mask exports retain their composition while avoiding redundant descendant exports.

The sandbox sends validated, typed messages with revision-correlated timing.
Binary asset transport preserves buffer identity without serializing repeated
image arrays. The UI conversion worker normalizes captured values and generates
Slint deterministically. It does not call Figma. Preview uses captured raster
geometry; native export generates editable text and supported component APIs.

Generated Slint is built through a small internal representation. Supported
Figma auto-layout maps to Slint FlexboxLayout. Component families retain sparse
variant selectors, dependency ordering and explicit variable bindings. Recoverable
unsupported properties are reported as approximations; structural failures remain
errors. Fixtures exercise conversion directly with JSON.

Native Dev Mode codegen has a separate startup path. It captures the callback's
node with root-only scope, then runs the same pure normalizer and converter in
the sandbox, without a UI or worker. This scope skips descendant capture, mask
composition and component-family expansion. Conversion projects the root before
validation and emits its element and appearance helpers without a preview wrapper.
Each callback owns its data; full-tree preview conversion still runs only in the
UI worker. Bundled sandbox tests guard both startup paths.

## Preview lifecycle

The controller owns WASM initialization, compilation and window replacement.
A new revision blanks preview/source/export immediately. Only the newest revision
may publish output. Unchanged source skips recompilation. Empty selection clears
without an error; capture or compilation errors keep output blank and select
Diagnostics. Resources from superseded work are released.

Pin state exists only in memory and resets on restart, page change or root deletion.
The sandbox observes relevant page/component changes and schedules captures without
artificial delays. Capture and UI timing retain the same revision and reconcile
wall time separately from overlapping work.

The source panel uses the inspector's Shiki grammar/themes and clipboard behavior.
Highlighting runs in a worker; copying uses raw source. Production omits development
snapshot and performance controls. Export uses the current presented revision.

## Build boundary

The standalone build validates an unmodified pinned Slint checkout and embeds the
interpreter, workers and syntax assets. Generated directories are build outputs.
The shipped manifest denies network access. Runtime preparation, artifact caching,
license notices and release-tag checks are standalone tooling; monorepo integration
will replace them with repository-native build inputs in a separate change.
