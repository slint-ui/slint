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

/// SharedPixelBuffer is a container for storing image data as pixels. It is
/// internally reference counted and cheap to clone.
///
/// You can construct a new empty shared pixel buffer with its default constructor,
/// or you can copy it from an existing contiguous buffer that you might already have, using that
/// constructor
///
/// See the documentation for Image for examples how to use this type to integrate
/// Slint with external rendering functions.
template<typename Pixel>
struct SharedPixelBuffer
{
    /// construct an empty SharedPixelBuffer
    SharedPixelBuffer() = default;

    /// construct a SharedPixelBuffer with the given size
    SharedPixelBuffer(uint32_t width, uint32_t height)
        : m_width(width), m_height(height), m_data(width * height)
    {
    }

    /// Construct a SharedPixelBuffer by copying the data from the \a data array.
    /// The array must be of size \a width * \a height
    SharedPixelBuffer(uint32_t width, uint32_t height, const Pixel *data)
        : m_width(width), m_height(height), m_data(data, data + (width * height))
    {
    }

    /// Return the width of the buffer in pixels
    uint32_t width() const { return m_width; }
    /// Return the height of the buffer in pixels
    uint32_t height() const { return m_height; }

    /// Returns a const pointer to the first pixel of this buffer.
    const Pixel *begin() const { return m_data.begin(); }
    /// Returns a const pointer past this buffer.
    const Pixel *end() const { return m_data.end(); }
    /// Returns a pointer to the first pixel of this buffer.
    Pixel *begin() { return m_data.begin(); }
    /// Returns a pointer past this buffer.
    Pixel *end() { return m_data.end(); }
    /// Returns a const pointer to the first pixel of this buffer.
    const Pixel *cbegin() const { return m_data.begin(); }
    /// Returns a const pointer past this buffer.
    const Pixel *cend() const { return m_data.end(); }

    /// Compare two SharedPixelBuffer. They are considered equal if all their pixels are equal
    bool operator==(const SharedPixelBuffer &other) const = default;

private:
    friend struct Image;
    uint32_t m_width;
    uint32_t m_height;
    SharedVector<Pixel> m_data;
};

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

    /// Construct an image from a SharedPixelBuffer of RGB pixel.
    Image(SharedPixelBuffer<Rgb8Pixel> buffer)
        : data(Data::ImageInner_EmbeddedImage(
                cbindgen_private::types::ImageCacheKey::Invalid(),
                cbindgen_private::types::SharedImageBuffer::RGB8(
                        cbindgen_private::types::SharedPixelBuffer<Rgb8Pixel> {
                                .width = buffer.width(),
                                .height = buffer.height(),
                                .data = buffer.m_data })))
    {
    }

    /// Construct an image from a SharedPixelBuffer of RGB pixel.
    Image(SharedPixelBuffer<Rgba8Pixel> buffer)
        : data(Data::ImageInner_EmbeddedImage(
                cbindgen_private::types::ImageCacheKey::Invalid(),
                cbindgen_private::types::SharedImageBuffer::RGBA8(
                        cbindgen_private::types::SharedPixelBuffer<Rgba8Pixel> {
                                .width = buffer.width(),
                                .height = buffer.height(),
                                .data = buffer.m_data })))
    {
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
