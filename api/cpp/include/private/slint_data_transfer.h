// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include <optional>
#include <utility>

#ifndef SLINT_FEATURE_FREESTANDING
#    include <any>
#endif

#include "private/slint_image.h"
#include "private/slint_string.h"
#include "private/slint_data_transfer_internal.h"

namespace slint {
/// `DataTransfer` abstracts over the various ways of transferring data within an application
/// and between applications. The details will depend on the current platform, but the common
/// features are:
///
/// - Each `DataTransfer` contains multiple views over the same data in different formats,
///   specified by MIME type
/// - The `DataTransfer` may contain an in-memory representation of the data, which can be
///   sent and received within the current application
/// - Serializing to a given format may be done eagerly or lazily
struct DataTransfer
{
public:
    /// Default constructor for `DataTransfer`
    DataTransfer() { cbindgen_private::types::slint_data_transfer_init_default(this); }

    /// Constructs a `DataTransfer` whose plain text representation is \a string.
    ///
    /// If \a string is empty, the resulting `DataTransfer` is empty (carries no plain
    /// text representation).
    explicit DataTransfer(const SharedString &string) : DataTransfer() { set_plain_text(string); }

    /// Constructs a `DataTransfer` whose image representation is \a image.
    /// Conversion to the relevant format is done on-demand.
    ///
    /// If \a image is default-constructed, the resulting `DataTransfer` is empty
    /// (carries no image representation).
    explicit DataTransfer(const Image &image) : DataTransfer() { set_image(image); }

    /// Destroys this `DataTransfer`, releasing any data it holds.
    ~DataTransfer() { cbindgen_private::types::slint_data_transfer_drop(this); }

    /// Creates a new `DataTransfer` that shares the data of \a other.
    DataTransfer(const DataTransfer &other)
    {
        cbindgen_private::types::slint_data_transfer_clone(this, &other);
    }

    /// Assigns \a other to this `DataTransfer` and returns a reference to this.
    DataTransfer &operator=(const DataTransfer &other)
    {
        if (this != &other) {
            cbindgen_private::types::slint_data_transfer_drop(this);
            cbindgen_private::types::slint_data_transfer_clone(this, &other);
        }
        return *this;
    }

    /// Move-constructs a `DataTransfer` from \a other, leaving \a other in a default-constructed
    /// state.
    DataTransfer(DataTransfer &&other) noexcept
    {
        cbindgen_private::types::slint_data_transfer_init_default(this);
        std::swap(_inner, other._inner);
    }

    /// Move-assigns \a other to this `DataTransfer` and returns a reference to this.
    DataTransfer &operator=(DataTransfer &&other) noexcept
    {
        std::swap(_inner, other._inner);
        return *this;
    }

    /// Sets the plain text representation of this `DataTransfer`.
    /// Each `DataTransfer` can only have a single plain text representation;
    /// calling this again overwrites the previous one.
    ///
    /// Passing an empty `text` clears the previously-set plain text instead of
    /// storing it.
    void set_plain_text(const SharedString &text)
    {
        cbindgen_private::types::slint_data_transfer_set_plain_text(this, &text);
    }

    /// Sets the image representation of this `DataTransfer`.
    /// Each `DataTransfer` can only have a single image representation;
    /// calling this again overwrites the previous one.
    ///
    /// Passing a default-constructed `Image` clears the previously-set image
    /// instead of storing it.
    void set_image(const Image &image)
    {
        cbindgen_private::types::slint_data_transfer_set_image(this, &image.data);
    }

    /// Returns `true` if this data transfer advertises a plain text representation.
    bool has_plain_text() const
    {
        return cbindgen_private::types::slint_data_transfer_has_plain_text(this);
    }

    /// Returns `true` if this data transfer advertises an image representation.
    bool has_image() const { return cbindgen_private::types::slint_data_transfer_has_image(this); }

    /// Returns `true` if this `DataTransfer` carries no data: no plain text, no image,
    /// and no user data.
    bool is_empty() const { return cbindgen_private::types::slint_data_transfer_is_empty(this); }

    /// Returns the plain text representation of this `DataTransfer`, or `std::nullopt` if no
    /// plain text representation is available.
    std::optional<SharedString> plain_text() const
    {
        SharedString out;
        if (cbindgen_private::types::slint_data_transfer_plain_text(this, &out)) {
            return out;
        }
        return std::nullopt;
    }

    /// Returns the image representation of this `DataTransfer`, or `std::nullopt` if no image
    /// representation is available.
    std::optional<Image> image() const
    {
        Image out;
        if (cbindgen_private::types::slint_data_transfer_image(this, &out.data)) {
            return out;
        }
        return std::nullopt;
    }

#if !defined(SLINT_FEATURE_FREESTANDING) || defined(DOXYGEN)
    /// Overload of `set_user_data()` for callers that already hold a `std::any`.
    void set_user_data(std::any value)
    {
        auto *box = new std::any(std::move(value));
        cbindgen_private::types::slint_data_transfer_set_user_data(
                this, box, +[](void *h) { delete static_cast<std::any *>(h); });
    }

    /// Returns the user data, or an empty `std::any` if none is set. Use `std::any_cast` to
    /// extract the concrete value.
    std::any user_data() const
    {
        const void *out = nullptr;
        if (!cbindgen_private::types::slint_data_transfer_user_data(this, &out)) {
            return {};
        }
        return *static_cast<const std::any *>(out);
    }
#endif

    /// Returns `true` if this `DataTransfer` holds user data.
    bool has_user_data() const
    {
        const void *out = nullptr;
        return cbindgen_private::types::slint_data_transfer_user_data(this, &out);
    }

    /// Clears the user data, if any.
    void clear_user_data() { cbindgen_private::types::slint_data_transfer_clear_user_data(this); }

    /// Compare two `DataTransfer` values for equality. This will return true if `b` is an
    /// unmodified clone of `a`, but if any modification has been done to either value since cloning
    /// then this will return false even if the two values are semantically identical.
    friend bool operator==(const DataTransfer &a, const DataTransfer &b)
    {
        return cbindgen_private::types::slint_data_transfer_eq(&a, &b);
    }

private:
    /// Storage matching the size and alignment of `DataTransfer` in `i_slint_core`.
    /// All operations on this field go through the `slint_data_transfer_*` FFI functions;
    /// it is never inspected directly from C++. See
    /// `i_slint_core::data_transfer::ffi::DataTransferOpaque`.
    cbindgen_private::types::DataTransferOpaque _inner;
};
}
