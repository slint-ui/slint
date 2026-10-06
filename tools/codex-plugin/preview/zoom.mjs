const levels = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];
const padding = 12;
const maxHeight = 420;

export function installPreviewZoom({ canvas, initialSize, layoutChanged }) {
  const viewport = document.getElementById("preview-scroll");
  const content = document.getElementById("preview-content");
  const stage = document.getElementById("preview-stage");
  let size = { width: initialSize.width, height: initialSize.height };
  let mode = "fit";
  let scale = 1;

  function fit() {
    return Math.max(0.01, Math.min(1, (viewport.clientWidth - 2 * padding) / size.width, (maxHeight - 2 * padding) / size.height));
  }
  function apply() {
    if (viewport.clientWidth === 0) return;
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
    viewport.style.height = Math.min(maxHeight, Math.ceil(size.height * scale + 2 * padding)) + "px";
    if (mode === "fit") { viewport.scrollLeft = 0; viewport.scrollTop = 0; }
    else {
      const after = canvas.getBoundingClientRect();
      viewport.scrollLeft += after.left + centerX * scale - viewBounds.left - viewport.clientWidth / 2;
      viewport.scrollTop += after.top + centerY * scale - viewBounds.top - viewport.clientHeight / 2;
    }
    viewport.dataset.zoom = String(scale);
    layoutChanged();
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
  new ResizeObserver(() => {
    if (viewport.clientWidth !== previousWidth) { previousWidth = viewport.clientWidth; apply(); }
  }).observe(viewport);
  apply();
  return {
    setSize(width, height) { size = { width, height }; apply(); },
    refresh: apply,
  };
}
