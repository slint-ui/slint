// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include <string_view>
#include <span>
#include "slint_generated_public.h"
#include "slint_size.h"
#include "slint_image_internal.h"
#include "slint_string.h"
#include "slint_sharedvector.h"

namespace slint {

/// SharedPixelBuffer is a container for storing image data as pixels. It is
/// internally reference counted and cheap to copy.
///
/// You can construct a new empty shared pixel buffer with its default constructor,
/// or you can copy it from an existing contiguous buffer that you might already have, using the
/// range constructor.
///
/// See the documentation for Image for examples how to use this type to integrate
/// Slint with external rendering functions.
template<typename Pixel>
struct SharedPixelBuffer
{
    /// Construct an empty SharedPixelBuffer.
    SharedPixelBuffer() = default;

    /// Construct a SharedPixelBuffer with the given \a width and \a height. The pixels are default
    /// initialized.
    SharedPixelBuffer(uint32_t width, uint32_t height)
        : m_width(width), m_height(height), m_data(width * height)
    {
    }

    /// Construct a SharedPixelBuffer by copying the data from the \a data array.
    /// The array must be of size \a width * \a height .
    SharedPixelBuffer(uint32_t width, uint32_t height, const Pixel *data)
        : m_width(width), m_height(height), m_data(data, data + (width * height))
    {
    }

    /// Returns the width of the buffer in pixels.
    uint32_t width() const { return m_width; }
    /// Returns the height of the buffer in pixels.
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

    /// Compare two SharedPixelBuffers. They are considered equal if all their pixels are equal.
    bool operator==(const SharedPixelBuffer &other) const = default;

private:
    friend struct Image;
    friend class Window;
    uint32_t m_width;
    uint32_t m_height;
    SharedVector<Pixel> m_data;
};

/// An image type that can be displayed by the Image element
///
/// You can construct Image objects from a path to an image file on disk, using
/// Image::load_from_path().
///
/// Another typical use-case is to render the image content with C++ code.
/// For this it’s most efficient to create a new SharedPixelBuffer with the known dimensions and
/// pass the pixel pointer returned by begin() to your rendering function. Afterwards you can create
/// an Image using the constructor taking a SharedPixelBuffer.
///
/// The following example creates a 320x200 RGB pixel buffer and calls a function to draw a shape
/// into it:
/// ```cpp
/// slint::SharedPixelBuffer::<slint::Rgb8Pixel> pixel_buffer(320, 200);
/// low_level_render(pixel_buffer.width(), pixel_buffer.height(),
///                  static_cast<unsigned char *>(pixel_buffer.begin()));
/// slint::Image image(pixel_buffer);
/// ```
///
/// Another use-case is to import existing image data into Slint, by
/// creating a new Image through copying of the buffer:
///
/// ```cpp
/// slint::Image image(slint::SharedPixelBuffer<slint::Rgb8Pixel>(the_width, the_height,
///     static_cast<slint::Rgb8Pixel*>(the_data));
/// ```
///
/// This only works if the static_cast is valid and the underlying data has the same
/// memory layout as slint::Rgb8Pixel or slint::Rgba8Pixel. Otherwise, you will have to do a
/// pixel conversion as you copy the pixels:
///
/// ```cpp
/// slint::SharedPixelBuffer::<slint::Rgb8Pixel> pixel_buffer(the_width, the_height);
/// slint::Rgb8Pixel *raw_data = pixel_buffer.begin();
/// for (int i = 0; i < the_width * the_height; i++) {
///   raw_data[i] = { bgr_data[i * 3 + 2], bgr_data[i * 3 + 1], bgr_data[i * 3] };
/// }
/// ```
struct Image
{
public:
    /// This enum describes the origin to use when rendering a borrowed OpenGL texture.
    enum class BorrowedOpenGLTextureOrigin {
        /// The top-left of the texture is the top-left of the texture drawn on the screen.
        TopLeft,
        /// The bottom-left of the texture is the top-left of the texture draw on the screen,
        /// flipping it vertically.
        BottomLeft,
    };

    Image() : data(Data::ImageInner_None()) { }

#if !defined(SLINT_FEATURE_FREESTANDING) || defined(DOXYGEN)
    /// Load an image from an image file
    [[nodiscard]] static Image load_from_path(const SharedString &file_path)
    {
        Image img;
        cbindgen_private::types::slint_image_load_from_path(&file_path, &img.data);
        return img;
    }
#endif

    /// Constructs a new Image from an existing OpenGL texture. The texture remains borrowed by
    /// Slint for the duration of being used for rendering, such as when assigned as source property
    /// to an `Image` element. It's the application's responsibility to delete the texture when it
    /// is not used anymore.
    ///
    /// The texture must be bindable against the `GL_TEXTURE_2D` target, have `GL_RGBA` as format
    /// for the pixel data.
    ///
    /// When Slint renders the texture, it assumes that the origin of the texture is at the
    /// top-left. This is different from the default OpenGL coordinate system. If you want to
    /// flip the origin, use BorrowedOpenGLTextureOrigin::BottomLeft.
    ///
    /// Safety:
    ///
    /// This function is unsafe because invalid texture ids may lead to undefined behavior in OpenGL
    /// drivers. A valid texture id is one that was created by the same OpenGL context that is
    /// current during any of the invocations of the callback set on
    /// [`Window::set_rendering_notifier()`]. OpenGL contexts between instances of [`slint::Window`]
    /// are not sharing resources. Consequently
    /// [`slint::Image`] objects created from borrowed OpenGL textures cannot be shared between
    /// different windows.
    [[nodiscard]] static Image create_from_borrowed_gl_2d_rgba_texture(
            uint32_t texture_id, Size<uint32_t> size,
            BorrowedOpenGLTextureOrigin origin = BorrowedOpenGLTextureOrigin::TopLeft)
    {
        cbindgen_private::types::BorrowedOpenGLTextureOrigin origin_private =
                origin == BorrowedOpenGLTextureOrigin::TopLeft
                ? cbindgen_private::types::BorrowedOpenGLTextureOrigin::TopLeft
                : cbindgen_private::types::BorrowedOpenGLTextureOrigin::BottomLeft;
        return Image(Data::ImageInner_BorrowedOpenGLTexture(
                cbindgen_private::types::BorrowedOpenGLTexture {
                        texture_id,
                        size,
                        origin_private,
                })

        );
    }

    /// Construct an image from a SharedPixelBuffer of RGB pixels.
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

    /// Construct an image from a SharedPixelBuffer of RGBA pixels.
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
    Size<uint32_t> size() const { return cbindgen_private::types::slint_image_size(&data); }

    /// Returns the path of the image on disk, if it was constructed via Image::load_from_path().
    std::optional<slint::SharedString> path() const
    {
        if (auto *str = cbindgen_private::types::slint_image_path(&data)) {
            return *str;
        } else {
            return {};
        }
    }

    /// Sets the nine-slice edges of the image.
    ///
    /// [Nine-slice scaling](https://en.wikipedia.org/wiki/9-slice_scaling) is a method for scaling
    /// images in such a way that the corners are not distorted.
    /// The arguments define the pixel sizes of the edges that cut the image into 9 slices.
    void set_nine_slice_edges(unsigned short top, unsigned short right, unsigned short bottom,
                              unsigned short left)
    {
        cbindgen_private::types::slint_image_set_nine_slice_edges(&data, top, right, bottom, left);
    }

    /// Returns the pixel buffer for the Image if available in RGB format without alpha.
    /// Returns nullopt if the pixels cannot be obtained, for example when the image was created
    /// from borrowed OpenGL textures.
    std::optional<SharedPixelBuffer<Rgb8Pixel>> to_rgb8() const
    {
        SharedPixelBuffer<Rgb8Pixel> result;
        if (cbindgen_private ::types::slint_image_to_rgb8(&data, &result.m_data, &result.m_width,
                                                          &result.m_height)) {
            return result;
        } else {
            return {};
        }
    }

    /// Returns the pixel buffer for the Image if available in RGBA format.
    /// Returns nullopt if the pixels cannot be obtained, for example when the image was created
    /// from borrowed OpenGL textures.
    std::optional<SharedPixelBuffer<Rgba8Pixel>> to_rgba8() const
    {
        SharedPixelBuffer<Rgba8Pixel> result;
        if (cbindgen_private ::types::slint_image_to_rgba8(&data, &result.m_data, &result.m_width,
                                                           &result.m_height)) {
            return result;
        } else {
            return {};
        }
    }

    /// Returns the pixel buffer for the Image if available in RGBA format, with the alpha channel
    /// pre-multiplied to the red, green, and blue channels. Returns nullopt if the pixels cannot be
    /// obtained, for example when the image was created from borrowed OpenGL textures.
    std::optional<SharedPixelBuffer<Rgba8Pixel>> to_rgba8_premultiplied() const
    {
        SharedPixelBuffer<Rgba8Pixel> result;
        if (cbindgen_private ::types::slint_image_to_rgba8_premultiplied(
                    &data, &result.m_data, &result.m_width, &result.m_height)) {
            return result;
        } else {
            return {};
        }
    }

    /// Returns true if \a a refers to the same image as \a b; false otherwise.
    friend bool operator==(const Image &a, const Image &b)
    {
        return cbindgen_private::types::slint_image_compare_equal(&a.data, &b.data);
    }
    /// Returns false if \a a refers to the same image as \a b; true otherwise.
    friend bool operator!=(const Image &a, const Image &b) { return !(a == b); }

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
            make_slice(data.data(), data.size()), string_to_slice(extension), &img);
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
