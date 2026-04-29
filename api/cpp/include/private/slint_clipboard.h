// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

namespace slint {
class ClipboardData
{
public:
    ClipboardData() = default;

    explicit ClipboardData(const slint::SharedString &string) { (void)string; }

    explicit ClipboardData(const slint::Image &image) { (void)image; }

    friend bool operator==(const ClipboardData &a, const ClipboardData &b) = default;

    bool hasPlaintext() const { return false; }

    bool hasImage() const { return false; }

    slint::SharedString readPlaintext()
    {
        slint::SharedString out = slint::SharedString();

        return out;
    }

    slint::Image readImage()
    {
        slint::Image out = slint::Image();

        return out;
    }
};
}
