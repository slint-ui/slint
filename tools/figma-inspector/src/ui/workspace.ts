// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export function mountWorkspace(): void {
    function element<T extends HTMLElement>(id: string): T {
        const found = document.getElementById(id);
        if (!found) throw Error(`Missing workspace element: ${id}`);
        return found as T;
    }
    const tabs = [element("preview-tab"), element("diagnostics-tab")];
    const panels = [
        element("preview-workspace"),
        element("diagnostics-workspace"),
    ];
    function select(index: number): void {
        tabs.forEach((tab, i) => {
            tab.setAttribute("aria-selected", String(i === index));
            tab.tabIndex = i === index ? 0 : -1;
            panels[i].hidden = i !== index;
        });
    }
    tabs.forEach((tab, index) => {
        tab.addEventListener("click", () => select(index));
        tab.addEventListener("keydown", (event) => {
            if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key))
                return;
            event.preventDefault();
            const next =
                event.key === "Home" ? 0 : event.key === "End" ? 1 : 1 - index;
            select(next);
            tabs[next].focus();
        });
    });
    const diagnostics = element("diagnostics");
    const badge = element("diagnostic-count");
    const empty = element("diagnostics-empty");
    const refresh = (): void => {
        const count = diagnostics.hidden
            ? 0
            : diagnostics.querySelectorAll("details > p").length || 1;
        badge.hidden = count === 0;
        badge.textContent = String(count);
        badge.dataset.severity = diagnostics.dataset.severity ?? "error";
        empty.hidden = count !== 0;
    };
    const observer = new MutationObserver(refresh);
    for (const target of [diagnostics]) {
        observer.observe(target, {
            childList: true,
            subtree: true,
            attributes: true,
            characterData: true,
        });
    }
    refresh();
}
const minHeight = 480;
const minWidth = 500;

type ResizeDirection = "right" | "bottom" | "bottomRight";
type WindowSize = { width: number; height: number };

interface DialogFrameProps {
    content: HTMLElement;
    initialHeight: number;
    initialWidth: number;
}

interface ResizeSession {
    direction: ResizeDirection;
    handle: HTMLElement;
    pointerId: number;
    startSize: WindowSize;
    startX: number;
    startY: number;
}

interface ResizeController {
    end: (
        handle: HTMLElement,
        pointerId: number | undefined,
        flush: boolean,
        event?: PointerEvent,
    ) => void;
    move: (handle: HTMLElement, event: PointerEvent) => void;
    start: (
        handle: HTMLElement,
        direction: ResizeDirection,
        event: PointerEvent,
    ) => void;
}

function resizeWindow(width: number, height: number): void {
    if (window.parent === window) return;
    window.parent.postMessage(
        { pluginMessage: { type: "resizeWindow", width, height } },
        "*",
    );
}

function clampSize(width: number, height: number): WindowSize {
    return {
        width: Math.min(4096, Math.max(minWidth, Math.round(width))),
        height: Math.min(4096, Math.max(minHeight, Math.round(height))),
    };
}

function sameSize(left: WindowSize | undefined, right: WindowSize): boolean {
    return left?.width === right.width && left.height === right.height;
}

function createResizeController(
    getSize: () => WindowSize,
    setSize: (size: WindowSize) => void,
): ResizeController {
    let activeSession: ResizeSession | undefined;
    let queuedSize: WindowSize | undefined;
    let frameRequest: number | undefined;
    let lastSentSize: WindowSize | undefined;

    const sendQueuedResize = (): void => {
        frameRequest = undefined;
        const size = queuedSize;
        queuedSize = undefined;
        if (size === undefined || sameSize(lastSentSize, size)) return;
        lastSentSize = size;
        setSize(size);
        resizeWindow(size.width, size.height);
    };

    const queueResize = (size: WindowSize): void => {
        const clampedSize = clampSize(size.width, size.height);
        if (sameSize(lastSentSize, clampedSize)) {
            if (frameRequest !== undefined) {
                window.cancelAnimationFrame(frameRequest);
                frameRequest = undefined;
            }
            queuedSize = undefined;
            return;
        }
        queuedSize = clampedSize;
        if (frameRequest === undefined) {
            frameRequest = window.requestAnimationFrame(sendQueuedResize);
        }
    };

    const cancelQueuedResize = (): void => {
        if (frameRequest !== undefined) {
            window.cancelAnimationFrame(frameRequest);
            frameRequest = undefined;
        }
        queuedSize = undefined;
    };

    const isActiveSession = (
        handle: HTMLElement,
        pointerId: number | undefined,
    ): boolean =>
        activeSession?.handle === handle &&
        (pointerId === undefined || activeSession.pointerId === pointerId);

    const finishResize = (flush: boolean): void => {
        const session = activeSession;
        if (session === undefined) return;
        if (flush) {
            if (frameRequest !== undefined) {
                window.cancelAnimationFrame(frameRequest);
                frameRequest = undefined;
            }
            sendQueuedResize();
        } else {
            cancelQueuedResize();
        }
        session.handle.dataset.active = "false";
        activeSession = undefined;
    };

    const sizeForPointer = (
        session: ResizeSession,
        event: PointerEvent,
    ): WindowSize => {
        const deltaX = event.clientX - session.startX;
        const deltaY = event.clientY - session.startY;
        return {
            width:
                session.direction === "bottom"
                    ? session.startSize.width
                    : session.startSize.width + deltaX,
            height:
                session.direction === "right"
                    ? session.startSize.height
                    : session.startSize.height + deltaY,
        };
    };

    return {
        start(handle, direction, event) {
            if (
                event.isPrimary === false ||
                event.button !== 0 ||
                activeSession !== undefined
            ) {
                return;
            }
            event.preventDefault();
            activeSession = {
                direction,
                handle,
                pointerId: event.pointerId,
                startSize: getSize(),
                startX: event.clientX,
                startY: event.clientY,
            };
            lastSentSize = activeSession.startSize;
            handle.dataset.active = "true";
            handle.setPointerCapture(event.pointerId);
        },

        move(handle, event) {
            const session = activeSession;
            if (
                session === undefined ||
                !isActiveSession(handle, event.pointerId)
            ) {
                return;
            }
            event.preventDefault();
            queueResize(sizeForPointer(session, event));
        },

        end(handle, pointerId, flush, event) {
            const session = activeSession;
            if (session === undefined || !isActiveSession(handle, pointerId)) {
                return;
            }
            if (flush && event !== undefined) {
                queueResize(sizeForPointer(session, event));
            }
            finishResize(flush);
        },
    };
}

function createResizeHandle(
    direction: ResizeDirection,
    frame: HTMLElement,
    controller: ResizeController,
): HTMLElement {
    const handle = document.createElement("div");
    const classDirection =
        direction === "bottomRight" ? "bottom-right" : direction;
    handle.className = `dialog-frame-resize-handle-${classDirection}`;
    handle.dataset.direction = direction;

    handle.addEventListener("pointerdown", (event: PointerEvent) => {
        controller.start(handle, direction, event);
    });

    handle.addEventListener("pointermove", (event: PointerEvent) => {
        controller.move(handle, event);
    });

    handle.addEventListener("pointerup", (event: PointerEvent) => {
        if (event.isPrimary === false || event.button !== 0) return;
        event.preventDefault();
        controller.end(handle, event.pointerId, true, event);
        if (handle.hasPointerCapture(event.pointerId)) {
            handle.releasePointerCapture(event.pointerId);
        }
    });

    handle.addEventListener("pointercancel", (event: PointerEvent) => {
        controller.end(handle, event.pointerId, false);
    });

    handle.addEventListener("lostpointercapture", () => {
        controller.end(handle, undefined, false);
    });

    frame.append(handle);
    return handle;
}

function DialogFrame({
    content,
    initialHeight,
    initialWidth,
}: DialogFrameProps): HTMLElement {
    const frame = document.createElement("div");
    frame.className = "dialog-frame-resizer";
    frame.style.height = `${initialHeight}px`;
    frame.style.width = `${initialWidth}px`;

    const contentHost = document.createElement("div");
    contentHost.className = "dialog-frame";
    contentHost.append(content);
    frame.append(contentHost);

    const getSize = (): WindowSize =>
        clampSize(window.innerWidth, window.innerHeight);
    const setSize = (size: WindowSize): void => {
        frame.style.width = `${size.width}px`;
        frame.style.height = `${size.height}px`;
    };
    const syncSizeToWindow = (): void => {
        setSize(getSize());
    };
    const controller = createResizeController(getSize, setSize);

    window.addEventListener("resize", syncSizeToWindow);
    syncSizeToWindow();
    createResizeHandle("right", frame, controller);
    createResizeHandle("bottom", frame, controller);
    createResizeHandle("bottomRight", frame, controller);
    return frame;
}

export function mountDialogFrame(content: HTMLElement): void {
    const rootElement = document.createElement("div");
    rootElement.id = "dialog-frame-root";
    content.before(rootElement);

    const isFigmaUi = window.parent !== window;
    rootElement.append(
        DialogFrame({
            content,
            initialWidth: isFigmaUi ? minWidth : window.innerWidth,
            initialHeight: isFigmaUi ? minHeight : window.innerHeight,
        }),
    );
}
