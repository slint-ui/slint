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

namespace sixtyfps {

struct ImageReference
{
public:
    ImageReference() : data(Data::None()) { }
    ImageReference(const SharedString &file_path) : data(Data::AbsoluteFilePath(file_path)) { }

    friend bool operator==(const ImageReference &a, const ImageReference &b) {
        return a.data == b.data;
    }
    friend bool operator!=(const ImageReference &a, const ImageReference &b) {
        return a.data != b.data;
    }


private:
    using Tag = cbindgen_private::types::ImageReference::Tag;
    using Data = cbindgen_private::types::ImageReference;
    Data data;
};

}
