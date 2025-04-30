// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

import React, { type ReactNode } from "react";

interface DialogFrameProps {
    children: ReactNode;
}

interface DialogSubComponentProps {
    children: ReactNode;
}

function DialogFrame({ children }: DialogFrameProps) {
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
        <div className="dialog-frame">
            <div className="main-content">
                {title}
                {content}
            </div>
            {footer}
        </div>
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
