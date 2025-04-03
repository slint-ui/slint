// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import React, { useEffect, useState } from "react";
import JSZip from "jszip";
import {
    listenTS,
    getColorTheme,
    subscribeColorTheme,
} from "./utils/bolt-utils";
import { getCopyToClipboard, getExportAll } from "./utils/utils.js";
import CodeSnippet from "./snippet/CodeSnippet";
import "./main.css";

// Add file download functionality
const downloadFile = (filename: string, text: string) => {
    const element = document.createElement('a');
    element.setAttribute('href', 'data:text/plain;charset=utf-8,' + encodeURIComponent(text));
    element.setAttribute('download', filename);
    element.style.display = 'none';
    document.body.appendChild(element);
    element.click();
    document.body.removeChild(element);
};

export const App = () => {
    const [exportsAreCurrent, setExportsAreCurrent] = useState(false);
    const [title, setTitle] = useState("");
    const [slintProperties, setSlintProperties] = useState("");
    const [exportedFiles, setExportedFiles] = useState<Array<{ name: string, content: string }>>([]);
    const [lightOrDarkMode, setLightOrDarkMode] = useState(getColorTheme());

    listenTS("updatePropertiesCallback", (res) => {
        setTitle(res.title || "");
        setSlintProperties(res.slintSnippet || "");
    });

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
                if (msg.type === "variableChanged" ||
                    msg.type === "variableCollectionChanged" ||
                    msg.type === "documentSnapshot") {

                    setExportsAreCurrent(false);
                }
            }
        };

        window.addEventListener("message", variableChangeHandler);
        return () => window.removeEventListener("message", variableChangeHandler);
    }, []);

    useEffect(() => {
        // Add specific variable change detection
        function setupVariableChangeDetection() {
            console.log("Setting up variable change detection...");

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
        const exportFilesHandler = (res: any) => {
            console.log("Received exportedFiles:", res.files);
            if (res.files && Array.isArray(res.files)) {
                console.log(`Setting ${res.files.length} files to state`);
                setExportedFiles(res.files);

                // Mark exports as current
                setExportsAreCurrent(true);

                console.log("Exports marked as current, files count:", res.files.length);
            } else {
                console.error("Invalid files data:", res);
            }
        };

        // Register the handler with listenTS
        listenTS("exportedFiles", exportFilesHandler);

        // Also add direct message listener as backup
        const directHandler = (event: MessageEvent) => {
            if (event.data.pluginMessage &&
                event.data.pluginMessage.type === 'exportedFiles') {
                console.log("DIRECT: Received exportedFiles via window message");
                exportFilesHandler(event.data.pluginMessage);
            }
        };

        window.addEventListener("message", directHandler);
        return () => window.removeEventListener("message", directHandler);
    }, []);

    // Function to communicate with the TypeScript side of the plugin
    function dispatchTS(eventName: string, payload: {}): void {
        // Send a message to the plugin code
        parent.postMessage({
            pluginMessage: {
                type: eventName,
                ...payload
            }
        }, '*');
    }

    // Create the functions with access to dispatchTS
    const copyToClipboardFn = getCopyToClipboard(dispatchTS);
    const downloadZipFile = async (files: Array<{ name: string, content: string }>) => {
        try {
            console.log("Creating ZIP with files:", files.map(f => `${f.name} (${f.content.length} bytes)`));

            if (!files || files.length === 0) {
                console.error("No files to zip!");
                return;
            }

            // Create a new JSZip instance directly (using the import)
            const zip = new JSZip();

            // Add each file to the zip with debug logging
            files.forEach(file => {
                console.log(`Adding to ZIP: ${file.name} (${file.content.length} bytes)`);
                zip.file(file.name, file.content);
            });

            // Generate the zip
            console.log("Generating ZIP blob...");
            const content = await zip.generateAsync({ type: "blob" });
            console.log(`ZIP created: ${content.size} bytes`);

            // Create download link
            const element = document.createElement('a');
            element.href = URL.createObjectURL(content);
            element.download = "figma-collections.zip";
            document.body.appendChild(element);
            element.click();
            document.body.removeChild(element);

            // Clean up
            URL.revokeObjectURL(element.href);

            console.log("ZIP file download initiated");
        } catch (error) {
            console.error("Error creating ZIP file:", error);

            // Fallback to individual downloads if ZIP creation fails
            alert("Couldn't create ZIP file. Downloading files individually...");
            files.forEach((file, index) => {
                setTimeout(() => {
                    downloadFile(file.name, file.content);
                }, index * 100);
            });
        }
    };

    // Add debugging log on each render
    console.log("Render state:", {
        exportedFilesCount: exportedFiles.length,
        exportsAreCurrent,
        hasProperties: slintProperties !== ""
    });

    return (
        <div className="container">
            <div className="title">
                {title || "Slint Figma Inspector"}
                {slintProperties !== "" && (
                    <div>
                        <span
                            id="copy-icon"
                            onClick={() => copyToClipboardFn(slintProperties)}
                            onKeyDown={() => copyToClipboardFn(slintProperties)}
                            className="copy-icon"
                        >
                            📋
                        </span>
                    </div>
                )}
            </div>
            <CodeSnippet
                code={slintProperties || "// Select a component to inspect"}
                theme={
                    lightOrDarkMode === "dark" ? "dark-slint" : "light-slint"
                }
            />
            <button
                onClick={() => dispatchTS("exportToFiles", {})}
                className="export-button"
                style={{
                    border: `none`,
                    margin: '4px 4px 12px 4px',
                    marginBottom: exportedFiles.length > 0 ? '4px' : '12px',
                    borderRadius: '4px',
                    background: lightOrDarkMode === "dark" ? '#4497F7' : '#4497F7',
                    padding: '4px',
                    width: '140px',
                    alignSelf: "center",
                    height: '32px',
                    color: 'white',
                }}
            >
                Export All Variables
            </button>

            <div style={{
                height: exportedFiles.length > 0 ? 'auto' : '0px',
                overflow: 'hidden',
                transition: 'all 0.3s ease-in-out'
            }}>
                {exportedFiles.length > 0 && (
                    <a
                        onClick={() => downloadZipFile(exportedFiles)}
                        style={{
                            backgroundColor: 'transparent',
                            color: lightOrDarkMode === "dark" ? 'white' : 'black',
                            marginTop: '4px',
                            marginBottom: '4px',
                            cursor: exportsAreCurrent ? 'pointer' : 'not-allowed',
                            fontSize: '0.8rem',
                            width: '100%',
                            display: 'flex',
                            justifyContent: 'center',
                            alignItems: 'center',
                            transition: 'all 0.3s ease',
                            opacity: exportsAreCurrent ? '1' : '0.5',
                        }}
                        // disabled={!exportsAreCurrent}
                    >
                        <span style={{ marginRight: '8px' }}>
                            {exportsAreCurrent ? '📦' : '⚠️'}
                        </span>
                        <span
                        style={{textDecoration: 'underline',
                        }}
                        >
                            {exportsAreCurrent
                            ? `Download ZIP (${exportedFiles.length} files)`
                            : 'Files outdated - export again'}
                        </span>
                    </a>
                )}
            </div>
        </div>
    );
};