// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "private/slint_image_internal.h"
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
    DataTransfer() = default;

    /// Explicit cast from `SharedString`, for plaintext
    explicit DataTransfer(const SharedString &string) { (void)string; }

    /// Explicit cast from `Image`, for any image type supported by Slint. Conversion to the
    /// relevant format is done on-demand.
    explicit DataTransfer(const Image &image) { (void)image; }

    /// Compare two `DataTransfer` values for equality. This will return true if `b` is an
    /// unmodified clone of `a`, but if any modification has been done to either value since cloning
    /// then this will return false even if the two values are semantically identical.
    friend bool operator==(const DataTransfer &a, const DataTransfer &b) = default;

private:
    /// Dummy pointers to ensure that this type is the same size as `DataTransfer` in
    /// `i_slint_core`. See `i_slint_core::data_transfer::ffi::DataTransferOpaque`.
    cbindgen_private::types::DataTransferOpaque _inner;
};
}
