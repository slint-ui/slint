// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

const levels = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];
const padding = 12;
const maxHeight = 420;

export function installPreviewZoom({ canvas, initialSize }) {
  const viewport = document.getElementById("preview-scroll");
  const content = document.getElementById("preview-content");
  const stage = document.getElementById("preview-stage");
  let size = { width: initialSize.width, height: initialSize.height };
  let mode = "fit";
  let scale = 1;
  let fullscreen = false;

  function fit() {
    const height = fullscreen ? viewport.clientHeight : maxHeight;
    return Math.max(0.01, Math.min(fullscreen ? Infinity : 1, (viewport.clientWidth - 2 * padding) / size.width, (height - 2 * padding) / size.height));
  }
  function apply() {
    if (fullscreen) viewport.style.height = "";
    if (viewport.clientWidth === 0 || (fullscreen && viewport.clientHeight === 0)) return;
    const previous = scale;
    const viewBounds = viewport.getBoundingClientRect();
    const before = canvas.getBoundingClientRect();
    const centerX = (viewBounds.left + viewport.clientWidth / 2 - before.left) / previous;
    const centerY = (viewBounds.top + viewport.clientHeight / 2 - before.top) / previous;
    scale = mode === "fit" ? fit() : Number(mode);
    stage.style.width = size.width + "px";
    stage.style.height = size.height + "px";
    content.style.width = size.width * scale + "px";
    content.style.height = size.height * scale + "px";
    stage.style.transform = `scale(${scale})`;
    if (!fullscreen) viewport.style.height = Math.min(maxHeight, Math.ceil(size.height * scale + 2 * padding)) + "px";
    if (mode === "fit") { viewport.scrollLeft = 0; viewport.scrollTop = 0; }
    else {
      const after = canvas.getBoundingClientRect();
      viewport.scrollLeft += after.left + centerX * scale - viewBounds.left - viewport.clientWidth / 2;
      viewport.scrollTop += after.top + centerY * scale - viewBounds.top - viewport.clientHeight / 2;
    }
  }
  function step(direction) {
    const level = direction > 0 ? levels.find(value => value > scale + 0.001) : levels.findLast(value => value < scale - 0.001);
    if (level === undefined) return;
    mode = String(level);
    apply();
  }
  viewport.addEventListener("keydown", event => {
    if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
    if (["+", "="].includes(event.key)) { event.preventDefault(); event.stopPropagation(); step(1); }
    else if (event.key === "-") { event.preventDefault(); event.stopPropagation(); step(-1); }
    else if (event.key === "0") { event.preventDefault(); event.stopPropagation(); mode = "1"; apply(); }
  }, true);
  viewport.addEventListener("wheel", event => {
    if (!(event.ctrlKey || event.metaKey) || event.deltaY === 0) return;
    event.preventDefault();
    event.stopPropagation();
    step(event.deltaY < 0 ? 1 : -1);
  }, { passive: false, capture: true });
  let previousWidth = 0;
  let previousHeight = 0;
  new ResizeObserver(() => {
    const width = viewport.clientWidth;
    const height = viewport.clientHeight;
    if (width !== previousWidth || (fullscreen && height !== previousHeight)) {
      previousWidth = width;
      previousHeight = height;
      apply();
    }
  }).observe(viewport);
  apply();
  return {
    setDisplayMode(displayMode) {
      const nextFullscreen = displayMode === "fullscreen";
      document.body.dataset.displayMode = nextFullscreen ? "fullscreen" : "inline";
      if (fullscreen === nextFullscreen) return;
      fullscreen = nextFullscreen;
      mode = "fit";
      apply();
    },
    setSize(width, height) { size = { width, height }; apply(); },
    refresh: apply,
  };
}
