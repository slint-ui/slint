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

using cbindgen_private::types::Rgb8Pixel;
using cbindgen_private::types::Rgba8Pixel;

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

    /// Construct an image from a vector of RGB pixel.
    /// The size of the vector \a data should be \a width * \a height
    static Image from_raw_data(unsigned int width, unsigned int height,
                               const SharedVector<Rgb8Pixel> &data)
    {
        Image img;
        img.data = Data::ImageInner_EmbeddedImage(
                cbindgen_private::types::ImageCacheKey::Invalid(),
                cbindgen_private::types::SharedImageBuffer::RGB8(
                        cbindgen_private::types::SharedPixelBuffer<Rgb8Pixel> {
                                .width = width, .height = height, .data = data }));
        return img;
    }

    /// Construct an image from a vector of RGBA pixels.
    /// The size of the vector \a data should be \a width * \a height
    static Image from_raw_data(unsigned int width, unsigned int height,
                               const SharedVector<Rgba8Pixel> &data)
    {
        Image img;
        img.data = Data::ImageInner_EmbeddedImage(
                cbindgen_private::types::ImageCacheKey::Invalid(),
                cbindgen_private::types::SharedImageBuffer::RGBA8(
                        cbindgen_private::types::SharedPixelBuffer<Rgba8Pixel> {
                                .width = width, .height = height, .data = data }));
        return img;
    }

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

namespace private_api {
inline Image load_image_from_embedded_data(std::span<const uint8_t> data,
                                           std::string_view extension)
{
    cbindgen_private::types::Image img(cbindgen_private::types::Image::ImageInner_None());
    cbindgen_private::types::slint_image_load_from_embedded_data(
            slint::cbindgen_private::Slice<uint8_t> { const_cast<uint8_t *>(data.data()),
                                                      data.size() },
            slint::cbindgen_private::Slice<uint8_t> {
                    const_cast<uint8_t *>(reinterpret_cast<const uint8_t *>(extension.data())),
                    extension.size() },
            &img);
    return Image(img);
}

inline Image image_from_embedded_textures(const cbindgen_private::types::StaticTextures *textures)
{
    cbindgen_private::types::Image img(cbindgen_private::types::Image::ImageInner_None());
    cbindgen_private::types::slint_image_from_embedded_textures(textures, &img);
    return Image(img);
}
}

}
