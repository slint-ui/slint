import { findButtonLabel, replaceButtonLabel, findButtonColor, replaceButtonColor } from "./label-edit.mjs";
export { installPreviewZoom } from "./zoom.mjs";

export function installLabelEditor({ canvas, getSource, changed }) {
  const toggle = document.getElementById("edit-toggle");
  const apply = document.getElementById("apply-edit");
  const cancel = document.getElementById("cancel-edit");
  const hit = document.getElementById("edit-target");
  const input = document.getElementById("label-input");
  const hint = document.getElementById("edit-hint");
  const badge = document.getElementById("draft-badge");
  const colorToggle = document.getElementById("color-toggle");
  const colorPanel = document.getElementById("color-panel");
  const colorProperty = document.getElementById("color-property");
  const colorInput = document.getElementById("color-input");
  const colorHex = document.getElementById("color-hex");
  const colorHint = document.getElementById("color-hint");
  let colorOpen = false;
  let enabled = false;
  let draft;
  let target;
  let compiledSource;
  let valid = false;
  let sending = false;
  let submitted = false;
  let notice = "";
  let nextId = 0;
  const requests = new Map();

  function activeSource() { return draft?.source ?? getSource().source; }
  function dirty() { return Boolean(draft && draft.source !== getSource().source); }
  function position() {
    if (!target || canvas.hidden) return;
    const source = getSource();
    const sx = canvas.clientWidth / source.width;
    const sy = canvas.clientHeight / source.height;
    const left = (target.x ?? (source.width - target.width) / 2) * sx;
    const top = (target.y ?? (source.height - target.height) / 2) * sy;
    Object.assign(hit.style, { left: left + "px", top: top + "px", width: target.width * sx + "px", height: target.height * sy + "px" });
    Object.assign(input.style, { left: left + 6 + "px", top: top + 6 + "px", width: Math.max(24, target.width * sx - 12) + "px", height: Math.max(20, target.height * sy - 12) + "px" });
    const appearance = findButtonLabel(activeSource()) ?? target;
    input.style.background = CSS.supports("color", appearance.background) ? appearance.background : "var(--code-surface)";
    input.style.color = CSS.supports("color", appearance.foreground) ? appearance.foreground : "var(--text)";
  }
  function refresh() {
    target = findButtonLabel(getSource().source);
    if (target && (target.width > getSource().width || target.height > getSource().height)) target = null;
    toggle.setAttribute("aria-pressed", String(enabled));
    toggle.disabled = sending || submitted;
    canvas.dataset.edit = String(enabled);
    canvas.inert = enabled;
    hit.hidden = !enabled || !target || canvas.hidden || sending || submitted;
    apply.hidden = !dirty() || submitted;
    cancel.hidden = !dirty() || submitted;
    apply.disabled = sending || !valid || compiledSource !== activeSource();
    cancel.disabled = sending;
    badge.hidden = !dirty();
    badge.textContent = submitted ? "Sent" : "Draft";
    colorToggle.hidden = !enabled;
    colorToggle.disabled = !target || sending || submitted;
    colorToggle.setAttribute("aria-expanded", String(colorOpen));
    colorPanel.hidden = !enabled || !colorOpen || !target || sending || submitted;
    const color = findButtonColor(activeSource(), colorProperty.value);
    colorInput.disabled = colorHex.disabled = !color;
    for (const swatch of colorPanel.querySelectorAll("[data-color]")) swatch.disabled = !color;
    if (color) {
      colorInput.value = color.color;
      if (document.activeElement !== colorHex) colorHex.value = color.color;
    } else colorHex.value = "";
    colorHint.textContent = color ? "Preview a color, then apply or cancel."
      : colorProperty.value === "background-color" && target?.properties.background
        ? "This button uses a background expression. Choose Label to edit its color."
        : "This color uses transparency or an expression. Ask Codex to change it.";
    hint.hidden = !enabled && !dirty() && !notice;
    hint.textContent = notice || (enabled
      ? target ? "Double-click the label or use Colors. Apply keeps changes; Cancel restores the source." : "Inline editing is unavailable for this preview. Ask Codex to change it."
      : "");
    position();
  }
  function discard() {
    const hadDraft = Boolean(draft);
    draft = undefined;
    submitted = false;
    notice = "";
    input.hidden = true;
    refresh();
    if (hadDraft) changed();
  }
  function begin() {
    if (!enabled || !target || sending || submitted) return;
    ensureDraft();
    colorOpen = false;
    input.value = draft.label;
    input.hidden = false;
    notice = "";
    refresh();
    input.focus();
    input.select();
  }
  function ensureDraft() {
    if (!draft) draft = { base: { ...getSource() }, source: getSource().source, target: { ...target }, label: target.label, colors: {} };
  }
  function rebuild() {
    let source = draft.base.source;
    if (draft.label !== draft.target.label) source = replaceButtonLabel(source, draft.target, draft.label);
    for (const [property, color] of Object.entries(draft.colors)) source = replaceButtonColor(source, findButtonColor(source, property), color);
    draft.source = source;
    notice = "";
    refresh();
    changed();
  }
  function changeColor(color) {
    if (!enabled || !target || sending || submitted) return;
    const original = findButtonColor(getSource().source, colorProperty.value);
    if (!original) return;
    if (findButtonColor(activeSource(), colorProperty.value)?.color === color.toLowerCase()) return;
    ensureDraft();
    if (color.toLowerCase() === original.color) delete draft.colors[colorProperty.value];
    else draft.colors[colorProperty.value] = color;
    rebuild();
  }
  colorToggle.addEventListener("click", () => {
    colorOpen = !colorOpen;
    input.hidden = true;
    if (colorOpen && !findButtonColor(activeSource(), colorProperty.value)) {
      const available = ["background-color", "label-color"].find(property => findButtonColor(activeSource(), property));
      if (available) colorProperty.value = available;
    }
    refresh();
    if (colorOpen) colorProperty.focus();
  });
  colorProperty.addEventListener("change", refresh);
  colorInput.addEventListener("input", () => changeColor(colorInput.value));
  colorHex.addEventListener("input", () => {
    const color = colorHex.value.trim();
    if (/^#[0-9a-f]{6}$/i.test(color)) changeColor(color);
  });
  colorHex.addEventListener("change", () => {
    const color = colorHex.value.trim();
    if (!/^#[0-9a-f]{6}$/i.test(color)) { colorHint.textContent = "Enter six hex digits, such as #2563eb."; return; }
    changeColor(color);
  });
  for (const swatch of colorPanel.querySelectorAll("[data-color]")) swatch.addEventListener("click", () => changeColor(swatch.dataset.color));
  colorPanel.addEventListener("keydown", event => {
    if (event.key === "Escape") { event.preventDefault(); discard(); colorOpen = false; refresh(); colorToggle.focus(); }
  });
  toggle.addEventListener("click", () => {
    if (sending || submitted) return;
    if (enabled && dirty()) discard();
    enabled = !enabled;
    colorOpen = false;
    input.hidden = true;
    notice = "";
    refresh();
    if (enabled && target) hit.focus();
  });
  hit.addEventListener("dblclick", begin);
  hit.addEventListener("keydown", event => {
    if (["Enter", "F2"].includes(event.key)) { event.preventDefault(); begin(); }
  });
  input.addEventListener("input", () => {
    if (!draft || sending || submitted) return;
    draft.label = input.value;
    rebuild();
  });
  input.addEventListener("keydown", event => {
    if (event.key === "Enter") { event.preventDefault(); input.hidden = true; hit.focus(); }
    if (event.key === "Escape") { event.preventDefault(); discard(); hit.focus(); }
  });
  input.addEventListener("blur", () => { input.hidden = true; });
  cancel.addEventListener("click", discard);

  function request(method, params) {
    return new Promise((resolve, reject) => {
      const id = "slint-edit-" + ++nextId;
      const timeout = setTimeout(() => { requests.delete(id); reject(new Error("The chat did not acknowledge the change. Check the chat before retrying.")); }, 10000);
      requests.set(id, { resolve, reject, timeout });
      window.parent.postMessage({ jsonrpc: "2.0", id, method, params }, "*");
    });
  }
  window.addEventListener("message", event => {
    if (event.source !== window.parent || event.data?.jsonrpc !== "2.0") return;
    const pending = requests.get(event.data.id);
    if (!pending || (!event.data.result && !event.data.error)) return;
    clearTimeout(pending.timeout);
    requests.delete(event.data.id);
    if (event.data.error || event.data.result?.isError) {
      const error = new Error(event.data.error?.message || "The chat rejected the edit request.");
      error.code = event.data.error?.code;
      pending.reject(error);
    } else pending.resolve(event.data.result);
  });
  apply.addEventListener("click", async () => {
    if (!dirty() || sending || submitted || !valid || compiledSource !== draft.source) return;
    const pending = draft;
    sending = true;
    input.hidden = true;
    notice = "Sending changes to Codex…";
    refresh();
    try {
      if (getSource().source !== pending.base.source || getSource().revision !== pending.base.revision) throw new Error("The source changed. Start the edit again.");
      const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(pending.base.source));
      const baseSourceHash = Array.from(new Uint8Array(digest), byte => byte.toString(16).padStart(2, "0")).join("");
      if (draft !== pending) throw new Error("The source changed. Start the edit again.");
      const edit = {
        sourcePath: pending.base.sourcePath, projectRoot: pending.base.projectRoot,
        baseRevision: pending.base.revision, baseSourceHash,
        target: { component: "Button", offset: pending.target.offset, property: "label" },
        before: pending.target.label, after: pending.label,
        baseSource: pending.base.source, source: pending.source,
        width: pending.base.width, height: pending.base.height,
        changes: [
          ...(pending.label !== pending.target.label ? [{ property: "label", before: pending.target.label, after: pending.label }] : []),
          ...Object.entries(pending.colors).map(([property, after]) => ({ property, before: findButtonColor(pending.base.source, property).value, after })),
        ],
      };
      edit.target.property = edit.changes.length === 1 ? edit.changes[0].property : "multiple";
      if (edit.changes.length === 1) { edit.before = edit.changes[0].before; edit.after = edit.changes[0].after; }
      if (window.parent === window) throw new Error("Open this preview inside the chat to send changes to Codex.");
      await request("ui/update-model-context", {
        structuredContent: { slintEdit: edit },
        content: [{ type: "text", text: "The user requested the Button property edits in slintEdit.changes. Apply them to the working Slint source when asked to apply the Slint changes. Preserve all other design and behavior. Check the base source hash before patching; reconcile changed source first. Validate with the configured LSP and render the new revision in this chat. Component source edits do not require publishing the plugin." }],
      });
      if (draft !== pending || getSource().source !== pending.base.source || getSource().revision !== pending.base.revision) {
        throw new Error("The source changed. Start the edit again.");
      }
      const prompt = "Apply my Slint changes.";
      try {
        await request("ui/message", { role: "user", content: [{ type: "text", text: prompt }] });
      } catch (error) {
        if (error.code !== -32601 || !window.openai?.sendFollowUpMessage) throw error;
        await window.openai.sendFollowUpMessage({ prompt });
      }
      if (draft === pending) {
        submitted = true;
        enabled = false;
        notice = "Sent to Codex. Look for the updated preview in the chat.";
      }
    } catch (error) {
      notice = error.message;
    } finally { sending = false; refresh(); }
  });
  new ResizeObserver(position).observe(canvas);
  refresh();
  return {
    source: activeSource,
    layoutChanged: position,
    isDraft: dirty,
    compiled(source, ok) { compiledSource = source; valid = ok; refresh(); },
    sourceChanged() {
      const hadDraft = Boolean(draft);
      draft = undefined;
      submitted = false;
      colorOpen = false;
      input.hidden = true;
      notice = hadDraft ? "Source updated. The previous draft was cleared." : "";
      refresh();
    },
  };
}
