// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import React, { type ReactNode } from "react";
import { Rnd } from "react-rnd";
import { useInspectorStore } from "../utils/store.js";

interface DialogFrameProps {
    children: ReactNode;
}

interface DialogSubComponentProps {
    children: ReactNode;
}

function DialogFrame({ children }: DialogFrameProps) {
    const { resizeWindow } = useInspectorStore();
    const childArray = React.Children.toArray(children);
    const title = childArray.find(
        (child) =>
            React.isValidElement(child) && child.type === DialogFrame.Title,
    );
    const content = childArray.find(
        (child) =>
            React.isValidElement(child) && child.type === DialogFrame.Content,
    );
    const footer = childArray.find(
        (child) =>
            React.isValidElement(child) && child.type === DialogFrame.Footer,
    );

    return (
        <Rnd
            default={{ x: 0, y: 0, width: 500, height: 320 }}
            style={{ minWidth: 500, minHeight: 320 }}
            disableDragging={true}
            onResize={(_e, _dir, refToElement) => {
                resizeWindow(parseInt(refToElement.style.width), parseInt(refToElement.style.height));
            }}
        >
            <div className="dialog-frame">
                <div className="main-content">
                    {title}
                    {content}
                </div>
                {footer}
            </div>
        </Rnd>
    );
}

DialogFrame.Title = function DialogTitle({
    children,
}: DialogSubComponentProps) {
    return <header className="dialog-frame-title">{children}</header>;
};

DialogFrame.Content = function DialogContent({
    children,
}: DialogSubComponentProps) {
    return <main className="dialog-frame-content">{children}</main>;
};

DialogFrame.Footer = function DialogFooter({
    children,
}: DialogSubComponentProps) {
    return <footer className="dialog-frame-footer">{children}</footer>;
};

export default DialogFrame;
