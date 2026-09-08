// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

export const FIRST_BUTTON_SOURCE = `export component Demo inherits Window {
    width: 240px;
    height: 72px;
    background: transparent;

    Rectangle {
        width: 100%;
        height: 100%;
        background: @linear-gradient(90deg, #2F80ED 0%, #9B51E0 100%);
        border-width: 1px;
        border-color: #1D4ED8;
        border-radius: 12px;
    }

    Text {
        width: 100%;
        height: 100%;
        color: white;
        text: "Slint";
        font-family: "Inter";
        font-size: 20px;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}
`;

export const SECOND_BUTTON_SOURCE = `export component Demo inherits Window {
    width: 240px;
    height: 72px;
    background: transparent;

    Rectangle {
        width: 100%;
        height: 100%;
        background: @linear-gradient(90deg, #F97316 0%, #EC4899 100%);
        border-width: 1px;
        border-color: #C2410C;
        border-radius: 12px;
    }

    Text {
        width: 100%;
        height: 100%;
        color: white;
        text: "Hot reloaded";
        font-family: "Inter";
        font-size: 20px;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}
`;
