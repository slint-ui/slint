// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { useEffect, useState, useRef } from "react";
import { getColorTheme, subscribeColorTheme } from "./utils/bolt-utils";
import CodeSnippet from "./components/snippet/CodeSnippet";
import { ExportType, useInspectorStore } from "./utils/store";
import DialogFrame from "./components/DialogFrame.js";
import { Text, Button, Checkbox, DropdownMenu } from "figma-kit";
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
    } = useInspectorStore();

    const [lightOrDarkMode, setLightOrDarkMode] = useState(getColorTheme());

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
        <>
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
                    </DropdownMenu.Root>
                    <Text
                        style={{
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
        </>
    );
};
