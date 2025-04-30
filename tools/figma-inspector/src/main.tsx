// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import { useEffect, useState, useRef } from "react";
import { getColorTheme, subscribeColorTheme} from "./utils/bolt-utils";
import CodeSnippet from "./snippet/CodeSnippet";
import { useInspectorStore } from "./utils/store";
import { downloadZipFile } from "./utils/utils.js";
import "./main.css";

export const App = () => {
    const {
        exportsAreCurrent,
        exportedFiles,
        title,
        slintSnippet,
        useVariables,
        exportAsSingleFile,
        menuOpen,
        copyToClipboard,
        initializeEventListeners,
        setUseVariables,
        setExportsAreCurrent,
        setExportAsSingleFile,
        setMenuOpen,
        toggleMenu,
        exportFiles,
    } = useInspectorStore();

    const [lightOrDarkMode, setLightOrDarkMode] = useState(getColorTheme());
    const menuRef = useRef<HTMLDivElement>(null); // Ref for the menu
    const buttonRef = useRef<HTMLButtonElement>(null); // Ref for the button

    useEffect(() => {
        // Only add listener if menu is open
        if (!menuOpen) {
            return;
        }

        const handleClickOutside = (event: MouseEvent) => {
            // Check if the click is outside the menu AND outside the button
            if (
                menuRef.current &&
                !menuRef.current.contains(event.target as Node) &&
                buttonRef.current && // Also check the button ref
                !buttonRef.current.contains(event.target as Node)
            ) {
                setMenuOpen(false); // Close the menu
            }
        };

        // Add listener on mount/when menu opens
        document.addEventListener("mousedown", handleClickOutside);

        // Cleanup listener on unmount/when menu closes
        return () => {
            document.removeEventListener("mousedown", handleClickOutside);
        };
    }, [menuOpen]); // Re-run effect when isMenuOpen changes

    // Theme handling
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

    const buttonStyle: React.CSSProperties = {
        border: `none`,
        margin: "4px 4px 0px 4px",
        borderRadius: "4px",
        background: lightOrDarkMode === "dark" ? "#4497F7" : "#4497F7",
        padding: "4px 8px",
        width: "auto",
        minWidth: "140px",
        alignSelf: "center",
        height: "32px",
        color: "white",
        cursor: "pointer",
        position: "relative",
        textAlign: "center",
        opacity: menuOpen ? 0.6 : 1,
        pointerEvents: menuOpen ? "none" : "auto",
    };

    const menuStyle: React.CSSProperties = {
        position: "absolute",
        bottom: "100%",
        left: "50%",
        transform: "translateX(-50%)",
        background: lightOrDarkMode === "dark" ? "#333" : "#fff",
        border: `1px solid ${lightOrDarkMode === "dark" ? "#555" : "#ccc"}`,
        borderRadius: "4px",
        boxShadow: "0 2px 5px rgba(0,0,0,0.2)",
        zIndex: 10,
        alignContent: "center",
        minWidth: "140px",
        padding: "5px 0",
        marginTop: "2px",
        justifyContent: "center",
        display: menuOpen ? "block" : "none",
    };

    const menuItemStyle: React.CSSProperties = {
        fontSize: "12px",
        cursor: "pointer",
        display: "flex",
        justifyContent: "center",
        alignItems: "center",
        color: lightOrDarkMode === "dark" ? "#eee" : "#333",
    };

    const menuItemHoverStyle: React.CSSProperties = {
        backgroundColor: lightOrDarkMode === "dark" ? "#444" : "#f0f0f0",
    };

    return (
        <div className="container">
            <div
                className="title"
                style={{
                    display: "flex",
                    alignItems: "center",
                    padding: "4px 8px",
                    borderBottom: `1px solid ${lightOrDarkMode === "dark" ? "#555" : "#ccc"}`,
                    flexShrink: 0,
                }}
            >
                <span
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
                >
                    📋
                </span>

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

                <div style={{ flexShrink: 0, marginLeft: "8px" }}>
                    {" "}
                    <div
                        style={{
                            display: "flex",
                            justifyContent: "flex-end",
                        }}
                    >
                        <label
                            style={{
                                cursor: "pointer",
                                fontSize: "12px",
                                display: "flex",
                                alignItems: "center",
                            }}
                        >
                            <input
                                type="checkbox"
                                checked={useVariables}
                                onChange={(e) =>
                                    setUseVariables(e.target.checked)
                                }
                                style={{
                                    marginRight: "4px",
                                    cursor: "pointer",
                                }}
                            />
                            Use Figma Variables
                        </label>
                    </div>
                </div>
            </div>{" "}
            <div
                style={{
                    flexGrow: 1,
                    overflowY: "auto",
                    minHeight: "50px",
                    position: "relative",
                }}
            >
                <CodeSnippet
                    code={slintSnippet || "// Select a component to inspect"}
                    theme={
                        lightOrDarkMode === "dark"
                            ? "dark-slint"
                            : "light-slint"
                    }
                />
            </div>
            <div
                style={{ position: "relative", alignSelf: "center" }}
                ref={menuRef}
            >
                {/* --- Trigger Button --- */}
                {useVariables && (
                    <button
                        ref={buttonRef}
                        onClick={toggleMenu} // Toggle menu visibility
                        style={buttonStyle}
                        className="export-button" // Keep class if needed
                    >
                        {"Design Tokens"}
                    </button>
                )}

                {/* --- Dropdown Menu --- */}
                {menuOpen && (
                    <div
                        ref={menuRef}
                        style={menuStyle}
                        className="export-dropdown-menu"
                    >
                        {/* Checkbox Item */}
                        <label
                            style={{ ...menuItemStyle, cursor: "pointer" }}
                            onMouseEnter={(e) =>
                                (e.currentTarget.style.backgroundColor =
                                    menuItemHoverStyle.backgroundColor!)
                            }
                            onMouseLeave={(e) =>
                                (e.currentTarget.style.backgroundColor = "")
                            }
                        >
                            <input
                                type="checkbox"
                                checked={exportAsSingleFile}
                                onChange={(e) =>
                                    setExportAsSingleFile(e.target.checked)
                                }
                                style={{
                                    marginRight: "8px",
                                    cursor: "pointer",
                                }}
                            />
                            Single Slint file
                        </label>

                        {/* Separator (Optional) */}
                        <hr
                            style={{
                                margin: "4px 0",
                                border: "none",
                                borderTop: `1px solid ${lightOrDarkMode === "dark" ? "#555" : "#ccc"}`,
                            }}
                        />

                        {/* Export Action Item */}
                        <div
                            role="button" // Semantics
                            tabIndex={0} // Make focusable
                            onClick={exportFiles}
                            onKeyDown={(e) => {
                                if (e.key === "Enter" || e.key === " ") {
                                    exportFiles();
                                }
                            }} // Keyboard accessibility
                            style={{ ...menuItemStyle, padding: "8px 12px" }}
                            onMouseEnter={(e) =>
                                (e.currentTarget.style.backgroundColor =
                                    menuItemHoverStyle.backgroundColor!)
                            }
                            onMouseLeave={(e) =>
                                (e.currentTarget.style.backgroundColor = "")
                            }
                        >
                            Export Collections
                        </div>
                    </div>
                )}
            </div>
            <div
                style={{
                    height: exportedFiles.length > 0 ? "auto" : "0px",
                    overflow: "hidden",
                    transition: "all 0.3s ease-in-out",
                }}
            >
                {exportedFiles.length > 0 && (
                    <a
                        onClick={() => downloadZipFile(exportedFiles)}
                        style={{
                            backgroundColor: "transparent",
                            color:
                                lightOrDarkMode === "dark" ? "white" : "black",
                            marginTop: "4px",
                            marginBottom: "4px",
                            cursor: exportsAreCurrent
                                ? "pointer"
                                : "not-allowed",
                            fontSize: "0.8rem",
                            width: "100%",
                            display: "flex",
                            justifyContent: "center",
                            alignItems: "center",
                            transition: "all 0.3s ease",
                            opacity: exportsAreCurrent ? "1" : "0.5",
                        }}
                        // disabled={!exportsAreCurrent}
                    >
                        <span style={{ marginRight: "8px" }}>
                            {exportsAreCurrent ? "📦" : "⚠️"}
                        </span>
                        <span style={{ textDecoration: "underline" }}>
                            {exportsAreCurrent
                                ? `Download ZIP (${exportedFiles.length} files)`
                                : "Files outdated - export again"}
                        </span>
                    </a>
                )}
            </div>
        </div>
    );
};
