// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

namespace slint {
struct Image;

class ClipboardData
{
public:
    explicit ClipboardData(const slint::SharedString &string)
    {
        (void)string;
    }

    explicit ClipboardData(const slint::Image &image)
    {
        (void)image;
    }

    friend bool operator==(const ClipboardData &a, const ClipboardData &b) = default;

    inline bool hasPlaintext()
    {
        return false;
    }

    inline bool hasImage()
    {
        return false;
    }

    inline slint::SharedString readPlaintext()
    {
        slint::SharedString out {};

        return out;
    }
};
}
