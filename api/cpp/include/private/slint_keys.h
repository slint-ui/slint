// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include "private/slint_keys_internal.h"
#include "private/slint_sharedvector.h"
#include "private/slint_string.h"

#include <optional>
#include <span>
#include <string_view>
#include <vector>

namespace slint {

class Keys;

namespace private_api {

void make_keys(Keys &out, const slint::SharedString &key, bool alt, bool control, bool shift,
               bool meta, bool ignoreShift, bool ignoreAlt, bool isPhysical);

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

    /// Create a `Keys` from a span of string parts, e.g. `{"Control", "Shift?", "Z"}`.
    ///
    /// Each element is either a modifier (`Control`, `Shift`, `Alt`, `Meta`, `Shift?`, `Alt?`)
    /// or a key name from the Key namespace (case-sensitive). If not found, it is treated as
    /// a string literal (must be a single lowercase grapheme cluster).
    ///
    /// Returns `std::nullopt` on parse failure.
    static std::optional<Keys> from_parts(std::span<const std::string_view> parts)
    {
        std::vector<SharedString> converted;
        converted.reserve(parts.size());
        for (const auto &sv : parts) {
            converted.emplace_back(sv);
        }
        Keys result;
        SharedString empty;
        cbindgen_private::Slice<SharedString> slice { converted.empty() ? &empty : converted.data(),
                                                      converted.size() };
        if (cbindgen_private::types::slint_keys_from_parts(slice, &result.data)) {
            return result;
        }
        return std::nullopt;
    }

    /// \overload
    static std::optional<Keys> from_parts(std::initializer_list<std::string_view> parts)
    {
        return from_parts(std::span<const std::string_view> { parts.begin(), parts.size() });
    }

    /// Decompose this `Keys` value into the list of string parts that
    /// `from_parts` accepts.
    ///
    /// Round-trips: `Keys::from_parts(keys.to_parts())` produces an equal `Keys`.
    ///
    /// An empty `Keys` returns an empty vector.
    SharedVector<SharedString> to_parts() const
    {
        SharedVector<SharedString> out;
        cbindgen_private::types::slint_keys_to_parts(&data, &out);
        return out;
    }

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
    // This way this class has the same binary representation and can be used interchangeably
    cbindgen_private::types::Keys data;

    /// \private
    /// Private constructor to construct a new Keys from \@keys(..)
    Keys(cbindgen_private::types::Keys &&data) : data(std::move(data)) { }
    friend void private_api::make_keys(Keys &out, const slint::SharedString &key, bool alt,
                                       bool control, bool shift, bool meta, bool ignoreShift,
                                       bool ignoreAlt, bool isPhysical);
};

namespace private_api {

// We need to use Keys& out so that we can forward-declare this and then use it as a friend
// Otherwise the size of `Keys` is not yet known.
inline void make_keys(Keys &out, const slint::SharedString &key, bool alt, bool control, bool shift,
                      bool meta, bool ignoreShift, bool ignoreAlt, bool isPhysical)
{
    ::slint::cbindgen_private::types::slint_keys(&key, alt, control, shift, meta, ignoreShift,
                                                 ignoreAlt, isPhysical, &out.data);
}

} // namespace private_api

} // namespace slint
