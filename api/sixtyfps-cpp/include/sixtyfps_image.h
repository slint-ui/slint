/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
#pragma once
#include <string_view>
#include "sixtyfps_image_internal.h"
#include "sixtyfps_string.h"
#include "sixtyfps_sharedvector.h"

namespace sixtyfps {

/// An image type that can be displayed by the Image element
struct Image
{
public:
    Image() : data(Data::None()) { }

    static Image load_from_path(const SharedString &file_path) {
        Image img;
        img.data = Data::AbsoluteFilePath(file_path);
        return img;
    }

    static Image load_from_argb(int width, int height, const SharedVector<uint32_t> &data) {
        Image img;
        img.data = Data::EmbeddedRgbaImage(width, height, data);
        return img;
    }

    friend bool operator==(const Image &a, const Image &b) {
        return a.data == b.data;
    }
    friend bool operator!=(const Image &a, const Image &b) {
        return a.data != b.data;
    }


private:
    using Tag = cbindgen_private::types::ImageReference::Tag;
    using Data = cbindgen_private::types::Image;
    Data data;
};

}
