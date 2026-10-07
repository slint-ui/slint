# Inline Slint previews

Use this workflow when `validate_slint` and `render_slint` are available.
They validate saved source with the bundled Slint LSP and preview it with the matching Wasm interpreter.
In CLI-only hosts, use the viewer and screenshot workflow instead.
A source-only plugin installation does not expose preview tools until its runtime is built or installed.

## Edit and preview

Keep `.slint` files as the source of truth and use `apply_patch` for edits.
Preserve existing project components, styling, and minimum or preferred dimensions.
Set explicit root dimensions only when the component has no usable size.
Keep a standalone preview Window transparent unless the user requests a background.
For standalone components, use the reported preview runtime version without inspecting unrelated project manifests.
For simple buttons, use the bundled starter described by `render_slint` and change only the requested properties.

Save, validate, and render in one execution:

1. Call `validate_slint` with the absolute entry `path`, a positive `revision`, and the project root when needed.
2. Continue only when `structuredContent.status` is `valid`.
   `error` means Slint diagnostics; `failure` means a validator or setup problem.
3. Call `render_slint` with that path, revision, matching `validatedProjectHash`, and logical canvas dimensions.
   Use the canonical `projectRoot` returned by validation.
After the render execution returns, call `get_preview_screenshot` in a separate execution with the returned `previewId`, revision, and source hash.
This gives the host an opportunity to open the preview before verification.
Inspect the image before claiming visual verification.
If capture is pending, wait briefly and retry without rendering again.
Limit retries to 15 seconds; report unavailable verification if no image arrives.

Relative imports and re-exports, PNG/JPEG/SVG/WebP images, and TTF/OTF fonts are supported.
Dependencies must stay inside the project root.
Network imports and include-path aliases are outside this prototype's supported scope.
Do not rebuild the plugin for ordinary component edits.

## Follow-up edits

Use the preview's source path, project root, revision, and hash to identify the saved entry.
Read the saved file and reconcile external edits before patching it.
Keep the same entry path and increment its revision.
After editing an imported component, validate and render the entry again with its original project root.
The project validation hash covers the entry, collected dependencies and assets, and the runtime revision.
Rendering rejects a project that changed after validation.

A render response confirms submission.
A matching `slintPreview.state` of `ready` acknowledges compilation, showing the instance, and a paint opportunity.
An `error` acknowledgement carries frontend diagnostics.
These acknowledgements may arrive asynchronously; do not claim visual verification from submission alone.
Ask the user to inspect the inline preview when it is not available to your inspection tools.

## Screenshot verification

The screenshot is captured from the inline Wasm canvas immediately after drawing the submitted source revision.
It includes Slint pixels without the host header, view controls, or CSS zoom.
Transparent pixels stay transparent; their displayed background depends on the image viewer.
It does not show later user interactions or changes in application state.
Check the image's source identity before reviewing layout, text, colors, and clipping.
A ready render acknowledgement confirms compilation and display, not appearance.
If the screenshot tool reports an error or an expired capture, say that visual verification was unavailable.
Do not use an image from another revision as evidence for the current edit.
