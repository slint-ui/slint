// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include "private/slint_events_internal.h"
#include "private/slint_string.h"

namespace slint {

class Keys;

namespace private_api {

void make_keys(Keys &out, const slint::SharedString &key, bool alt, bool control, bool shift,
               bool meta, bool ignoreShift, bool ignoreAlt);

} // namespace private_api

/// A `Keys` is created by the \@keys(...) macro in Slint and
/// defines which key event(s) activate a KeyBinding.
class Keys
{
public:
    /// Returns an empty `keys` instance, which never matches any key event.
    Keys() = default;
    /// Copy constructor
    Keys(const Keys &) = default;
    /// Move constructor
    Keys(Keys &&) = default;

    /// Copy assignment operator
    slint::Keys &operator=(const Keys &) = default;
    /// Move assignment operator
    slint::Keys &operator=(Keys &&) = default;

    /// Equality operator, returns true if the two `Keys` instances are equal, i.e. they match the
    /// same key events.
    friend bool operator==(const Keys &a, const Keys &b) { return a.data == b.data; }
    /// Inequality operator, returns true if the two `Keys` instances are not equal, i.e. they match
    /// different key events.
    friend bool operator!=(const Keys &a, const Keys &b) { return a.data != b.data; }

    /// Returns a string that looks native on the current platform.
    ///
    /// For example, the shortcut created with \@keys(Meta + Control + A)
    /// will be converted like this:
    /// - **macOS**: `⌃⌘A`
    /// - **Windows**: `Win+Ctrl+A`
    /// - **Linux**: `Super+Ctrl+A`
    ///
    /// Note that this functions output is best-effort and may be adjusted/improved at any time,
    /// do not rely on this output to be stable!
    inline SharedString to_string() const
    {
        SharedString out;
        cbindgen_private::types::slint_keys_to_string(&data, &out);
        return out;
    }

private:
    // Use only one instance of the actual Rust struct as the inner data.
    // This way this class has the same binary representation and can be used interchangibly
    cbindgen_private::types::Keys data;

    /// \private
    /// Private constructor to construct a new Keys from \@keys(..)
    Keys(cbindgen_private::types::Keys &&data) : data(std::move(data)) { }
    friend void private_api::make_keys(Keys &out, const slint::SharedString &key, bool alt,
                                       bool control, bool shift, bool meta, bool ignoreShift,
                                       bool ignoreAlt);
};

namespace private_api {

// We need to use Keys& out so that we can forward-declare this and then use it as a friend
// Otherwise the size of `Keys` is not yet known.
inline void make_keys(Keys &out, const slint::SharedString &key, bool alt, bool control, bool shift,
                      bool meta, bool ignoreShift, bool ignoreAlt)
{
    ::slint::cbindgen_private::types::slint_keys(&key, alt, control, shift, meta, ignoreShift,
                                                 ignoreAlt, &out.data);
}

} // namespace private_api

} // namespace slint
