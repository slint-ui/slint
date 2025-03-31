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
    const [title, setTitle] = useState("");
    const [slintProperties, setSlintProperties] = useState("");

    listenTS("updatePropertiesCallback", (res) => {
        setTitle(res.title || "");
        setSlintProperties(res.slintSnippet || "");
    });

    const [lightOrDarkMode, setLightOrDarkMode] = useState(getColorTheme());
    useEffect(() => {
        subscribeColorTheme((mode) => {
            setLightOrDarkMode(mode);
        });
    }, []);

    // Add these checks to verify message flow
    useEffect(() => {
        // Raw message listener to see everything coming from the plugin
        interface PluginMessage {
            type: string;
            [key: string]: any;
        }

        interface PluginEventData {
            pluginMessage?: PluginMessage;
        }

        const debugMessageHandler = (event: MessageEvent<PluginEventData>) => {
            if (event.data.pluginMessage) {
                console.log("DEBUG: Raw message from plugin:", event.data.pluginMessage);
            }
        };
        window.addEventListener("message", debugMessageHandler);
        return () => window.removeEventListener("message", debugMessageHandler);
    }, []);

    // Add a state for exported files
    const [exportedFiles, setExportedFiles] = useState<Array<{ name: string, content: string }>>([]);

    useEffect(() => {
        // Define interfaces for type-safety
        interface ExportedFile {
            name: string;
            content: string;
        }

        interface ExportedFilesPayload {
            files?: ExportedFile[];
            [key: string]: any;
        }

        const handler = (res: ExportedFilesPayload): void => {
            console.log("Received exportedFiles:", res.files);
            if (res.files && Array.isArray(res.files)) {
                console.log(`Setting ${res.files.length} files to state`);
                setExportedFiles(res.files);

                // Force UI update
                setTimeout(() => {
                    console.log("After setState, length:", res.files?.length ?? 0);
                    // Force re-render by setting a state var
                    setTitle(prev => prev + " ");
                }, 100);
            } else {
                console.error("Invalid files data:", res);
            }
        };

        listenTS("exportedFiles", handler);

        // Also add direct message listener as backup
        const directHandler = (event: MessageEvent<{ pluginMessage?: { type: string, files?: any } }>) => {
            if (event.data.pluginMessage &&
                event.data.pluginMessage.type === 'exportedFiles') {
                console.log("DIRECT: Received exportedFiles:",
                    event.data.pluginMessage.files?.length || 0);
                handler(event.data.pluginMessage);
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
    const exportAllFn = getExportAll(dispatchTS);
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

    return (
        <div className="container">
            <div className="title">
                {title}
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
                code={slintProperties}
                theme={
                    lightOrDarkMode === "dark" ? "dark-slint" : "light-slint"
                }
            />
            <button
                onClick={() => dispatchTS("exportToFiles", {})}
                className="export-button"
            >
                Export All Collections to Files
            </button>
            {exportedFiles.length > 0 && (
            <button
                    onClick={() => downloadZipFile(exportedFiles)}
                    style={{
                        backgroundColor: '#2196F3',
                        color: 'white',
                        border: 'none',
                        padding: '10px',
                        marginTop: '10px',
                        cursor: 'pointer',
                        borderRadius: '4px',
                        fontWeight: 'bold',
                        width: '100%',
                        display: 'flex',
                        justifyContent: 'center',
                        alignItems: 'center'
                    }}
                >
                    <span style={{marginRight: '8px'}}>📦</span> Download All as ZIP ({exportedFiles.length} files)
                </button>
            )}
        </div>

    );
};

