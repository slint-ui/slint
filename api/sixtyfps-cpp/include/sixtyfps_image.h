// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

#pragma once
#include <string_view>
#include "sixtyfps_image_internal.h"
#include "sixtyfps_string.h"
#include "sixtyfps_sharedvector.h"

namespace sixtyfps {

#if !defined(DOXYGEN)
using cbindgen_private::types::Size;
#else
/// The Size structure is used to represent a two-dimensional size
/// with width and height.
struct Size
{
    /// The width of the size
    float width;
    /// The height of the size
    float height;
};
#endif

/// An image type that can be displayed by the Image element
struct Image
{
public:
    Image() : data(Data::None()) { }

    /// Load an image from an image file
    static Image load_from_path(const SharedString &file_path)
    {
        Image img;
        img.data = Data::AbsoluteFilePath(file_path);
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
    Size size() const { return cbindgen_private::types::sixtyfps_image_size(&data); }

    /// Returns the path of the image on disk, if it was constructed via Image::load_from_path().
    std::optional<sixtyfps::SharedString> path() const
    {
        if (auto *str = cbindgen_private::types::sixtyfps_image_path(&data)) {
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
