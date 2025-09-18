// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once
#include <string_view>
#include <span>
#include "slint_string_internal.h"

namespace slint {

/// A string type used by the Slint run-time.
///
/// SharedString uses implicit data sharing to make it efficient to pass around copies. When
/// copying, a reference to the data is cloned, not the data itself.
///
/// The class provides constructors from std::string_view as well as the automatic conversion to
/// a std::string_view.
///
/// For convenience, it's also possible to convert a number to a string using
/// SharedString::from_number(double).
///
/// Under the hood the string data is UTF-8 encoded and it is always terminated with a null
/// character.
struct SharedString
{
    /// Creates an empty default constructed string.
    SharedString() { cbindgen_private::slint_shared_string_from_bytes(this, "", 0); }
    /// Creates a new SharedString from the string view \a s. The underlying string data
    /// is copied.
    SharedString(std::string_view s)
    {
        cbindgen_private::slint_shared_string_from_bytes(this, s.data(), s.size());
    }
    /// Creates a new SharedString from the null-terminated string pointer \a s. The underlying
    /// string data is copied. It is assumed that the string is UTF-8 encoded.
    SharedString(const char *s) : SharedString(std::string_view(s)) { }
    /// Creates a new SharedString from the null-terminated string pointer \a s. The underlying
    /// string data is copied.
    SharedString(const char8_t *s) : SharedString(reinterpret_cast<const char *>(s)) { }
    /// Creates a new SharedString from the string view \a s. The underlying string data is copied.
    SharedString(std::u8string_view s)
    {
        cbindgen_private::slint_shared_string_from_bytes(
                this, reinterpret_cast<const char *>(s.data()), s.size());
    }
    /// Creates a new SharedString from \a other.
    SharedString(const SharedString &other)
    {
        cbindgen_private::slint_shared_string_clone(this, &other);
    }
    /// Destroys this SharedString and frees the memory if this is the last instance
    /// referencing it.
    ~SharedString() { cbindgen_private::slint_shared_string_drop(this); }
    /// Assigns \a other to this string and returns a reference to this string.
    SharedString &operator=(const SharedString &other)
    {
        cbindgen_private::slint_shared_string_drop(this);
        cbindgen_private::slint_shared_string_clone(this, &other);
        return *this;
    }
    /// Assigns the string view \a s to this string and returns a reference to this string.
    /// The underlying string data is copied.  It is assumed that the string is UTF-8 encoded.
    SharedString &operator=(std::string_view s)
    {
        cbindgen_private::slint_shared_string_drop(this);
        cbindgen_private::slint_shared_string_from_bytes(this, s.data(), s.size());
        return *this;
    }
    /// Assigns null-terminated string pointer \a s to this string and returns a reference
    /// to this string. The underlying string data is copied. It is assumed that the string
    /// is UTF-8 encoded.
    SharedString &operator=(const char *s) { return *this = std::string_view(s); }

    /// Move-assigns \a other to this SharedString instance.
    SharedString &operator=(SharedString &&other)
    {
        std::swap(inner, other.inner);
        return *this;
    }

    /// Provides a view to the string data. The returned view is only valid as long as at
    /// least this SharedString exists.
    operator std::string_view() const { return cbindgen_private::slint_shared_string_bytes(this); }
    /// Provides a raw pointer to the string data. The returned pointer is only valid as long as at
    /// least this SharedString exists.
    auto data() const -> const char * { return cbindgen_private::slint_shared_string_bytes(this); }
    /// Size of the string, in bytes. This excludes the terminating null character.
    std::size_t size() const { return std::string_view(*this).size(); }

    /// Returns a pointer to the first character. It is only safe to dereference the pointer if the
    /// string contains at least one character.
    const char *begin() const { return data(); }
    /// Returns a point past the last character of the string. It is not safe to dereference the
    /// pointer, but it is suitable for comparison.
    const char *end() const
    {
        std::string_view view(*this);
        return view.data() + view.size();
    }

    /// \return true if the string contains no characters; false otherwise.
    bool empty() const { return std::string_view(*this).empty(); }

    /// \return true if the string starts with the specified prefix string; false otherwise
    bool starts_with(std::string_view prefix) const
    {
        return std::string_view(*this).substr(0, prefix.size()) == prefix;
    }

    /// \return true if the string ends with the specified prefix string; false otherwise
    bool ends_with(std::string_view prefix) const
    {
        std::string_view self_view(*this);
        return self_view.size() >= prefix.size()
                && self_view.compare(self_view.size() - prefix.size(), std::string_view::npos,
                                     prefix)
                == 0;
    }

    /// Reset to an empty string
    void clear() { *this = std::string_view("", 0); }

    /// Creates a new SharedString from the given number \a n. The string representation of the
    /// number uses a minimal formatting scheme: If \a n has no fractional part, the number will be
    /// formatted as an integer.
    ///
    /// For example:
    /// \code
    ///     auto str = slint::SharedString::from_number(42); // creates "42"
    ///     auto str2 = slint::SharedString::from_number(100.5) // creates "100.5"
    /// \endcode
    static SharedString from_number(double n) { return SharedString(n); }

    /// Returns the lowercase equivalent of this string, as a new SharedString.
    ///
    /// For example:
    /// \code
    ///     auto str = slint::SharedString("Hello");
    ///     auto str2 = str.to_lowercase(); // creates "hello"
    /// \endcode
    SharedString to_lowercase() const
    {
        auto out = SharedString();
        cbindgen_private::slint_shared_string_to_lowercase(&out, this);
        return out;
    }

    /// Returns the uppercase equivalent of this string, as a new SharedString.
    ///
    /// For example:
    /// \code
    ///     auto str = slint::SharedString("Hello");
    ///     auto str2 = str.to_uppercase(); // creates "HELLO"
    /// \endcode
    SharedString to_uppercase() const
    {
        auto out = SharedString();
        cbindgen_private::slint_shared_string_to_uppercase(&out, this);
        return out;
    }

    /// Returns true if \a a is equal to \a b; otherwise returns false.
    friend bool operator==(const SharedString &a, const SharedString &b)
    {
        return std::string_view(a) == std::string_view(b);
    }
    /// Returns true if \a a is not equal to \a b; otherwise returns false.
    friend bool operator!=(const SharedString &a, const SharedString &b)
    {
        return std::string_view(a) != std::string_view(b);
    }

    /// Returns true if \a a is lexicographically less than \a b; false otherwise.
    friend bool operator<(const SharedString &a, const SharedString &b)
    {
        return std::string_view(a) < std::string_view(b);
    }
    /// Returns true if \a a is lexicographically less or equal than \a b; false otherwise.
    friend bool operator<=(const SharedString &a, const SharedString &b)
    {
        return std::string_view(a) <= std::string_view(b);
    }
    /// Returns true if \a a is lexicographically greater than \a b; false otherwise.
    friend bool operator>(const SharedString &a, const SharedString &b)
    {
        return std::string_view(a) > std::string_view(b);
    }
    /// Returns true if \a a is lexicographically greater or equal than \a b; false otherwise.
    friend bool operator>=(const SharedString &a, const SharedString &b)
    {
        return std::string_view(a) >= std::string_view(b);
    }

    /// Writes the \a shared_string to the specified \a stream and returns a reference to the
    /// stream.
    friend std::ostream &operator<<(std::ostream &stream, const SharedString &shared_string)
    {
        return stream << std::string_view(shared_string);
    }

    /// Concatenates \a a and \a and returns the result as a new SharedString.
    friend SharedString operator+(const SharedString &a, std::string_view b)
    {
        SharedString a2 = a;
        return a2 += b;
    }
    /// Move-concatenates \a b to \a and returns a reference to \a a.
    friend SharedString operator+(SharedString &&a, std::string_view b)
    {
        a += b;
        return a;
    }
    /// Appends \a other to this string and returns a reference to this.
    SharedString &operator+=(std::string_view other)
    {
        cbindgen_private::slint_shared_string_append(this, other.data(), other.size());
        return *this;
    }

private:
    /// Use SharedString::from_number
    explicit SharedString(double n) { cbindgen_private::slint_shared_string_from_number(this, n); }
    void *inner; // opaque
};

namespace private_api {

template<typename T>
inline cbindgen_private::Slice<T> make_slice(const T *ptr, size_t len)
{
    return cbindgen_private::Slice<T> {
        // Rust uses a NonNull, so even empty slices shouldn't use nullptr
        .ptr = ptr ? const_cast<T *>(ptr) : reinterpret_cast<T *>(sizeof(T)),
        .len = len,
    };
}

template<typename T, size_t Extent>
inline cbindgen_private::Slice<std::remove_const_t<T>> make_slice(std::span<T, Extent> span)
{
    return make_slice(span.data(), span.size());
}

inline cbindgen_private::Slice<uint8_t> string_to_slice(std::string_view str)
{
    return make_slice(reinterpret_cast<const uint8_t *>(str.data()), str.size());
}
}

}
