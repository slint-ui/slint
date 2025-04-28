// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import {
    useEffect,
    useState,
    useCallback,
    useRef,
    type ReactNode,
} from "react";
import JSZip from "jszip";
import {
    dispatchTS,
    listenTS,
    getColorTheme,
    subscribeColorTheme,
} from "./utils/bolt-utils";
import { copyToClipboard } from "./utils/utils.js";
import CodeSnippet from "./snippet/CodeSnippet";
import "./main.css";

// Add file download functionality
const downloadFile = (filename: string, text: string) => {
    const element = document.createElement("a");
    element.setAttribute(
        "href",
        "data:text/plain;charset=utf-8," + encodeURIComponent(text),
    );
    element.setAttribute("download", filename);
    element.style.display = "none";
    document.body.appendChild(element);
    element.click();
    document.body.removeChild(element);
};

export const App = () => {
    const [exportsAreCurrent, setExportsAreCurrent] = useState(false);
    const [title, setTitle] = useState("");
    const [slintProperties, setSlintProperties] = useState("");
    const [exportedFiles, setExportedFiles] = useState<
        Array<{ name: string; content: string }>
    >([]);
    const [lightOrDarkMode, setLightOrDarkMode] = useState(getColorTheme());
    // State for the export format toggle
    const [exportAsSingleFile, setExportAsSingleFile] = useState(false); // Default to multiple files
    //  Add state for dropdown visibility
    const [isMenuOpen, setIsMenuOpen] = useState(false);
    const menuRef = useRef<HTMLDivElement>(null); // Ref for the menu
    const buttonRef = useRef<HTMLButtonElement>(null); // Ref for the button
    const toggleMenu = useCallback(() => {
        setIsMenuOpen((prev) => !prev);
    }, []);
    const handleExportClick = useCallback(() => {
        setExportedFiles([]);
        setExportsAreCurrent(false);
        dispatchTS("exportToFiles", { exportAsSingleFile: exportAsSingleFile });
        setIsMenuOpen(false); // Close menu after clicking export
    }, [exportAsSingleFile]);
    const [useVariables, setUseVariables] = useState(false); // Default to false

    listenTS("updatePropertiesCallback", (res) => {
        setTitle(res.title);
        setSlintProperties(res.slintSnippet || "");
    });

    useEffect(() => {
        const handleSelectionChange = () => {
            // Request snippet update using the current state of useVariables
            dispatchTS("generateSnippetRequest", {
                useVariables: useVariables,
            });
        };
        listenTS("selectionChangedInFigma", handleSelectionChange);

        // Also request initial snippet on component mount
        dispatchTS("generateSnippetRequest", { useVariables: useVariables });
    }, [useVariables]);
    useEffect(() => {
        // Only add listener if menu is open
        if (!isMenuOpen) {
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
                setIsMenuOpen(false); // Close the menu
            }
        };

        // Add listener on mount/when menu opens
        document.addEventListener("mousedown", handleClickOutside);

        // Cleanup listener on unmount/when menu closes
        return () => {
            document.removeEventListener("mousedown", handleClickOutside);
        };
    }, [isMenuOpen]); // Re-run effect when isMenuOpen changes
    const handleCheckboxChange = useCallback(
        (event: React.ChangeEvent<HTMLInputElement>) => {
            const checked = event.target.checked;
            setExportAsSingleFile(checked);
            // Keep menu open when checkbox is toggled
        },
        [useVariables],
    );
    // Update the handleUseVariables handler
    const handleUseVariables = useCallback(
        (event: React.ChangeEvent<HTMLInputElement>) => {
            const checked = event.target.checked;
            // Update the state
            setUseVariables(checked);
            // Request a new snippet from the backend with the updated preference
            dispatchTS("generateSnippetRequest", { useVariables: checked });
        },
        [],
    );

    // Theme handling
    useEffect(() => {
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

    useEffect(() => {
        // Add specific variable change detection
        function setupVariableChangeDetection() {
            // Request the plugin to start monitoring variable changes
            dispatchTS("monitorVariableChanges", { enabled: true });
        }

        // Call it on component mount
        setupVariableChangeDetection();

        // Also poll periodically to check for changes
        const intervalId = setInterval(() => {
            dispatchTS("checkVariableChanges", {});
        }, 5000); // Check every 5 seconds

        return () => {
            clearInterval(intervalId);
            // Disable monitoring when component unmounts
            dispatchTS("monitorVariableChanges", { enabled: false });
        };
    }, []);

    // Export files handler
    useEffect(() => {
        const exportFilesHandler = async (res: any) => {
            // Make the handler async
            if (res.files && Array.isArray(res.files) && res.files.length > 0) {
                // Ensure files exist
                setExportedFiles(res.files);

                // Mark exports as current
                setExportsAreCurrent(true);

                //  Automatically trigger download
                await downloadZipFile(res.files); // Call downloadZipFile with the received files
                //  End automatic download
            } else {
                console.error("Invalid or empty files data received:", res);
                // Reset state if files are invalid/empty after an export attempt
                setExportedFiles([]);
                setExportsAreCurrent(false); // Mark as not current if export failed to produce files
            }
        };

        // Register the handler with listenTS
        listenTS("exportedFiles", exportFilesHandler);

        // Also add direct message listener as backup
        const directHandler = (event: MessageEvent) => {
            if (
                event.data.pluginMessage &&
                event.data.pluginMessage.type === "exportedFiles"
            ) {
                exportFilesHandler(event.data.pluginMessage); // Call the same async handler
            }
        };

        window.addEventListener("message", directHandler);
        return () => window.removeEventListener("message", directHandler);
    }, []);

    // Create the functions with access to dispatchTS
    const downloadZipFile = async (
        files: Array<{ name: string; content: string }>,
    ) => {
        try {
            if (!files || files.length === 0) {
                console.error("No files to zip!");
                return;
            }

            // Create a new JSZip instance directly (using the import)
            const zip = new JSZip();

            // Add each file to the zip with debug logging
            files.forEach((file) => {
                zip.file(file.name, file.content);
            });

            // Generate the zip
            const content = await zip.generateAsync({ type: "blob" });

            // Create download link
            const element = document.createElement("a");
            element.href = URL.createObjectURL(content);
            element.download = "figma-collections.zip";
            document.body.appendChild(element);
            element.click();
            document.body.removeChild(element);

            // Clean up
            URL.revokeObjectURL(element.href);
        } catch (error) {
            console.error("Error creating ZIP file:", error);

            // Fallback to individual downloads if ZIP creation fails
            alert(
                "Couldn't create ZIP file. Downloading files individually...",
            );
            files.forEach((file, index) => {
                setTimeout(() => {
                    downloadFile(file.name, file.content);
                }, index * 100);
            });
        }
    };

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
        opacity: isMenuOpen ? 0.6 : 1,
        pointerEvents: isMenuOpen ? "none" : "auto",
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
        display: isMenuOpen ? "block" : "none",
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
                    onClick={() => copyToClipboard(slintProperties)}
                    onKeyDown={(e) => {
                        if (e.key === "Enter" || e.key === " ") {
                            copyToClipboard(slintProperties);
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
                                onChange={handleUseVariables}
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
                    code={slintProperties || "// Select a component to inspect"}
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
                        {exportsAreCurrent ? "Design Tokens" : "Design Tokens"}
                    </button>
                )}

                {/* --- Dropdown Menu --- */}
                {isMenuOpen && (
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
                                onChange={handleCheckboxChange}
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
                            onClick={handleExportClick}
                            onKeyDown={(e) => {
                                if (e.key === "Enter" || e.key === " ") {
                                    handleExportClick();
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
