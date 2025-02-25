// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import React, { useEffect, useState } from "react";
import {
    listenTS,
    getColorTheme,
    subscribeColorTheme,
} from "./utils/bolt-utils";
import { copyToClipboard } from "./utils/utils.js";
import CodeSnippet from "./snippet/CodeSnippet";
import "./main.css";

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

    return (
        <div className="container">
            <div className="title">
                {title}
                {slintProperties !== "" && (
                    <span
                        id="copy-icon"
                        onClick={() => copyToClipboard(slintProperties)}
                        onKeyDown={() => copyToClipboard(slintProperties)}
                        className="copy-icon"
                    >
                        📋
                    </span>
                )}
            </div>
            <CodeSnippet
                code={slintProperties}
                theme={
                    lightOrDarkMode === "dark" ? "dark-slint" : "light-slint"
                }
            />
        </div>
    );
};
