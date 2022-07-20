// Copyright © SixtyFPS GmbH <info@slint-ui.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-commercial

#pragma once
#include <string_view>
#include "slint_generated_public.h"
#include "slint_size.h"
#include "slint_image_internal.h"
#include "slint_string.h"
#include "slint_sharedvector.h"

namespace slint {

/// An image type that can be displayed by the Image element
struct Image
{
public:
    Image() : data(Data::ImageInner_None()) { }

    /// Load an image from an image file
    static Image load_from_path(const SharedString &file_path)
    {
        Image img;
        cbindgen_private::types::slint_image_load_from_path(&file_path, &img.data);
        return img;
    }

    /*
    static Image load_from_argb(int width, int height, const SharedVector<uint32_t> &data) {
        Image img;
        img.data = Data::EmbeddedRgbaImage(width, height, data);
        return img;
    }
    */

    /// Returns the size of the Image in pixels.
    Size<unsigned int> size() const { return cbindgen_private::types::slint_image_size(&data); }

    /// Returns the path of the image on disk, if it was constructed via Image::load_from_path().
    std::optional<slint::SharedString> path() const
    {
        if (auto *str = cbindgen_private::types::slint_image_path(&data)) {
            return *str;
        } else {
            return {};
        }
    }

    /// Returns true if \a a refers to the same image as \a b; false otherwise.
    friend bool operator==(const Image &a, const Image &b) { return a.data == b.data; }
    /// Returns false if \a a refers to the same image as \a b; true otherwise.
    friend bool operator!=(const Image &a, const Image &b) { return a.data != b.data; }

    /// \private
    explicit Image(cbindgen_private::types::Image inner) : data(inner) { }

private:
    using Tag = cbindgen_private::types::ImageInner::Tag;
    using Data = cbindgen_private::types::Image;
    Data data;
};

}
