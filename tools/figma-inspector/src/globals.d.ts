// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

export type Message = {
    type: string; // Allow the 'type' property (required)
    [key: string]: any; // Allow any other properties spread from 'data'
};

export interface PluginMessageEvent {
    pluginMessage: Message;
    pluginId?: string;
}

declare module "*.png";
declare module "*.gif";
declare module "*.jpg";
declare module "*.svg";
