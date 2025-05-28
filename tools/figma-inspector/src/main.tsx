// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { useEffect, useState } from "react";
import { getColorTheme, subscribeColorTheme } from "./utils/bolt-utils";
import CodeSnippet from "./components/snippet/CodeSnippet";
import { ExportType, useInspectorStore } from "./utils/store";
import DialogFrame from "./components/DialogFrame.js";
import { Button, Checkbox, DropdownMenu, Text } from "figma-kit";
import "./main.css";

export const App = () => {
    const {
        exportsAreCurrent,
        title,
        slintSnippet,
        useVariables,
        copyToClipboard,
        initializeEventListeners,
        setUseVariables,
        setExportsAreCurrent,
        exportFiles,
        resizeWindow,
        windowWidth,
        windowHeight,
        setWindowDimensions,
    } = useInspectorStore();

    const [_lightOrDarkMode, setLightOrDarkMode] = useState(getColorTheme());
    const [isResizing, setIsResizing] = useState(false);
    const [resizeDirection, setResizeDirection] = useState<string>("");
    const [startPos, setStartPos] = useState({ x: 0, y: 0 });
    const [startDimensions, setStartDimensions] = useState({
        width: 0,
        height: 0,
    });

    // Handle resize start
    const handleResizeStart = (direction: string) => (e: React.MouseEvent) => {
        e.preventDefault();
        setIsResizing(true);
        setResizeDirection(direction);
        setStartPos({ x: e.clientX, y: e.clientY });
        setStartDimensions({ width: windowWidth, height: windowHeight });
    };

    // Handle mouse events during resize
    useEffect(() => {
        const handleMouseMove = (e: MouseEvent) => {
            if (!isResizing) {
                return;
            }

            const deltaX = e.clientX - startPos.x;
            const deltaY = e.clientY - startPos.y;

            let newWidth = startDimensions.width;
            let newHeight = startDimensions.height;

            // Calculate new dimensions based on resize direction
            if (resizeDirection.includes("e")) {
                newWidth = startDimensions.width + deltaX;
            }
            if (resizeDirection.includes("s")) {
                newHeight = startDimensions.height + deltaY;
            }

            // Apply minimum constraints
            newWidth = Math.max(400, newWidth);
            newHeight = Math.max(300, newHeight);

            setWindowDimensions(newWidth, newHeight);
            resizeWindow(newWidth, newHeight);
        };

        const handleMouseUp = () => {
            setIsResizing(false);
            setResizeDirection("");
        };

        if (isResizing) {
            document.addEventListener("mousemove", handleMouseMove);
            document.addEventListener("mouseup", handleMouseUp);
        }

        return () => {
            document.removeEventListener("mousemove", handleMouseMove);
            document.removeEventListener("mouseup", handleMouseUp);
        };
    }, [
        isResizing,
        startPos,
        startDimensions,
        resizeDirection,
        resizeWindow,
        setWindowDimensions,
    ]);

    // Listen for window resize events
    useEffect(() => {
        const handleResize = () => {
            const width = window.innerWidth;
            const height = window.innerHeight;
            setWindowDimensions(width, height);
        };

        window.addEventListener("resize", handleResize);
        return () => window.removeEventListener("resize", handleResize);
    }, [setWindowDimensions]);

    // Init
    useEffect(() => {
        initializeEventListeners();
        subscribeColorTheme((mode) => {
            setLightOrDarkMode(mode);
        });
    }, []);

    // Debug listener
    useEffect(() => {
        const variableChangeHandler = (event: any) => {
            if (event.data?.pluginMessage) {
                const msg = event.data.pluginMessage;

                // Check for variable-specific event types
                if (
                    msg.type === "variableChanged" ||
                    msg.type === "variableCollectionChanged" ||
                    msg.type === "documentSnapshot"
                ) {
                    setExportsAreCurrent(false);
                }
            }
        };

        window.addEventListener("message", variableChangeHandler);
        return () =>
            window.removeEventListener("message", variableChangeHandler);
    }, []);

    return (
        <div
            style={{
                width: "100%",
                height: "100vh",
                position: "relative",
                display: "flex",
                flexDirection: "column",
            }}
        >
            <DialogFrame>
                <DialogFrame.Title>
                    <svg
                        id="copy-icon"
                        onClick={() => copyToClipboard()}
                        onKeyDown={(e) => {
                            if (e.key === "Enter" || e.key === " ") {
                                copyToClipboard();
                            }
                        }}
                        className="copy-icon"
                        style={{ cursor: "pointer", marginRight: "8px" }}
                        role="button"
                        tabIndex={0}
                        width="24"
                        height="24"
                        fill="none"
                        viewBox="0 0 24 24"
                    >
                        <path
                            fill="var(--color-icon)"
                            fill-rule="evenodd"
                            d="M10 6h4v1h-4zM9 6a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1 2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H9a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2m0 1a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h6a1 1 0 0 0 1-1V8a1 1 0 0 0-1-1 1 1 0 0 1-1 1h-4a1 1 0 0 1-1-1m1 3.5a.5.5 0 0 1 .5-.5h3a.5.5 0 0 1 0 1h-3a.5.5 0 0 1-.5-.5m.5 2.5a.5.5 0 0 0 0 1h3a.5.5 0 0 0 0-1z"
                            clip-rule="evenodd"
                        ></path>
                    </svg>
                    <span
                        style={{
                            whiteSpace: "nowrap",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            flexGrow: 1,
                            textAlign: "left",
                        }}
                    >
                        {title || "Slint Figma Inspector"}
                    </span>
                </DialogFrame.Title>
                <DialogFrame.Content>
                    <CodeSnippet
                        code={
                            slintSnippet || "// Select a component to inspect"
                        }
                    />
                </DialogFrame.Content>
                <DialogFrame.Footer>
                    <Checkbox.Root>
                        <Checkbox.Input
                            checked={useVariables}
                            onChange={(e) => setUseVariables(e.target.checked)}
                        />
                        <Checkbox.Label>Use Figma Variables</Checkbox.Label>
                    </Checkbox.Root>
                    <DropdownMenu.Root>
                        <DropdownMenu.Trigger asChild>
                            <Button
                                variant={
                                    exportsAreCurrent ? "secondary" : "primary"
                                }
                                style={{
                                    visibility: useVariables
                                        ? "visible"
                                        : "hidden",
                                }}
                            >
                                Export
                            </Button>
                        </DropdownMenu.Trigger>
                        <DropdownMenu.Content>
                            <DropdownMenu.Item
                                onClick={() =>
                                    exportFiles(ExportType.SeparateFiles)
                                }
                            >
                                Separate Files Per Collection…
                            </DropdownMenu.Item>
                            <DropdownMenu.Item
                                onClick={() =>
                                    exportFiles(ExportType.SingleFile)
                                }
                            >
                                Single Design-Tokens File…
                            </DropdownMenu.Item>
                        </DropdownMenu.Content>
                    </DropdownMenu.Root>{" "}
                    <Text
                        style={{
                            paddingRight: "20px",
                            color: exportsAreCurrent
                                ? "var(--figma-color-text-disabled)"
                                : "var(--figma-color-text)",
                        }}
                    >
                        {useVariables ? (
                            exportsAreCurrent ? (
                                <em>Exports are current</em>
                            ) : (
                                "Either variables have changed or no export found"
                            )
                        ) : (
                            ""
                        )}
                    </Text>
                </DialogFrame.Footer>
            </DialogFrame>

            {/* Resize handles - edges */}
            {/* Bottom edge */}
            <div
                onMouseDown={handleResizeStart("s")}
                style={{
                    position: "absolute",
                    bottom: 0,
                    left: "4px",
                    right: "4px",
                    height: "4px",
                    cursor: "s-resize",
                    zIndex: 1000,
                }}
            />
            {/* Right edge */}
            <div
                onMouseDown={handleResizeStart("e")}
                style={{
                    position: "absolute",
                    right: 0,
                    top: "4px",
                    bottom: "4px",
                    width: "4px",
                    cursor: "e-resize",
                    zIndex: 1000,
                }}
            />

            {/* Resize handles - corners */}
            {/* Top-left corner */}
            <div
                onMouseDown={handleResizeStart("nw")}
                style={{
                    position: "absolute",
                    top: 0,
                    left: 0,
                    width: "8px",
                    height: "8px",
                    cursor: "nw-resize",
                    zIndex: 1001,
                }}
            />
            {/* Top-right corner */}
            <div
                onMouseDown={handleResizeStart("ne")}
                style={{
                    position: "absolute",
                    top: 0,
                    right: 0,
                    width: "8px",
                    height: "8px",
                    cursor: "ne-resize",
                    zIndex: 1001,
                }}
            />
            {/* Bottom-left corner */}
            <div
                onMouseDown={handleResizeStart("sw")}
                style={{
                    position: "absolute",
                    bottom: 0,
                    left: 0,
                    width: "8px",
                    height: "8px",
                    cursor: "sw-resize",
                    zIndex: 1001,
                }}
            />
            {/* Bottom-right corner */}
            <div
                onMouseDown={handleResizeStart("se")}
                style={{
                    position: "absolute",
                    bottom: 0,
                    right: 0,
                    width: "8px",
                    height: "8px",
                    cursor: "se-resize",
                    zIndex: 1001,
                }}
            />
        </div>
    );
};
