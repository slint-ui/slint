// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

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
