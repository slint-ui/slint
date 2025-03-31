// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include <cstddef>
#include <new>
#include <algorithm>
#include <optional>
#include <atomic>

#ifdef __APPLE__
#    include <AvailabilityMacros.h>
#endif

namespace vtable {

template<typename T>
struct VRefMut
{
    const T *vtable;
    void *instance;
};

struct Layout
{
    std::size_t size;
    std::size_t align;
};

// For the C++'s purpose, they are all the same
template<typename T>
using VRef = VRefMut<T>;

template<typename T>
using Pin = T;

template<typename T>
struct VBox
{
    const T *vtable = nullptr;
    void *instance = nullptr;
    explicit VBox(const T *vtable, void *instance) : vtable(vtable), instance(instance) { }
    VBox(const VBox &) = delete;
    VBox() = default;
    VBox &operator=(const VBox &) = delete;
    ~VBox()
    {
        if (vtable && instance) {
            vtable->drop({ vtable, instance });
        }
    }
};

struct AllowPin;

template<typename Base, typename T, typename Flag = void>
struct VOffset
{
    const T *vtable;
    std::uintptr_t offset;
};

template<typename VTable, typename X>
struct VRcInner
{
    template<typename VTable_, typename X_>
    friend class VRc;
    template<typename VTable_, typename X_>
    friend class VWeak;

private:
    VRcInner() : layout {} { }
    const VTable *vtable = &X::static_vtable;
    std::atomic<int> strong_ref = 1;
    std::atomic<int> weak_ref = 1;
    std::uint16_t data_offset = offsetof(VRcInner, data);
    union {
        X data;
        Layout layout;
    };

    void *data_ptr() { return reinterpret_cast<char *>(this) + data_offset; }
    ~VRcInner() = delete;
};

struct Dyn
{
};

template<typename VTable, typename X = Dyn>
class VRc
{
    VRcInner<VTable, X> *inner;
    VRc(VRcInner<VTable, X> *inner) : inner(inner) { }
    template<typename VTable_, typename X_>
    friend class VWeak;

public:
    ~VRc()
    {
        if (!--inner->strong_ref) {
            Layout layout = inner->vtable->drop_in_place({ inner->vtable, &inner->data });
            layout.size += inner->data_offset;
            layout.align = std::max<size_t>(layout.align, alignof(VRcInner<VTable, Dyn>));
            inner->layout = layout;
            if (!--inner->weak_ref) {
                inner->vtable->dealloc(inner->vtable, reinterpret_cast<uint8_t *>(inner), layout);
            }
        }
    }
    VRc(const VRc &other) : inner(other.inner) { inner->strong_ref++; }
    VRc &operator=(const VRc &other)
    {
        if (inner == other.inner)
            return *this;
        this->~VRc();
        new (this) VRc(other);
        return *this;
    }
    /// Construct a new VRc holding an X.
    ///
    /// The type X must have a static member `static_vtable` of type VTable
    template<typename... Args>
    static VRc make(Args... args)
    {
#if !defined(__APPLE__) || MAC_OS_X_VERSION_MIN_REQUIRED >= MAC_OS_X_VERSION_10_14
        auto mem = ::operator new(sizeof(VRcInner<VTable, X>),
                                  static_cast<std::align_val_t>(alignof(VRcInner<VTable, X>)));
#else
        auto mem = ::operator new(sizeof(VRcInner<VTable, X>));
#endif
        auto inner = new (mem) VRcInner<VTable, X>;
        new (&inner->data) X(args...);
        return VRc(inner);
    }

    const X *operator->() const { return &inner->data; }
    const X &operator*() const { return inner->data; }
    X *operator->() { return &inner->data; }
    X &operator*() { return inner->data; }

    const VRc<VTable, Dyn> &into_dyn() const
    {
        return *reinterpret_cast<const VRc<VTable, Dyn> *>(this);
    }

    VRef<VTable> borrow() const { return { inner->vtable, inner->data_ptr() }; }

    friend bool operator==(const VRc &a, const VRc &b) { return a.inner == b.inner; }
    friend bool operator!=(const VRc &a, const VRc &b) { return a.inner != b.inner; }
    const VTable *vtable() const { return inner->vtable; }
};

template<typename VTable, typename X = Dyn>
class VWeak
{
    VRcInner<VTable, X> *inner = nullptr;

public:
    VWeak() = default;
    ~VWeak()
    {
        if (inner && !--inner->weak_ref) {
            inner->vtable->dealloc(inner->vtable, reinterpret_cast<uint8_t *>(inner),
                                   inner->layout);
        }
    }
    VWeak(const VWeak &other) : inner(other.inner) { inner && inner->weak_ref++; }
    VWeak(const VRc<VTable, X> &other) : inner(other.inner) { inner && inner->weak_ref++; }
    VWeak &operator=(const VWeak &other)
    {
        if (inner == other.inner)
            return *this;
        this->~VWeak();
        new (this) VWeak(other);
        return *this;
    }

    std::optional<VRc<VTable, X>> lock() const
    {
        if (!inner || inner->strong_ref == 0)
            return {};
        inner->strong_ref++;
        return { VRc<VTable, X>(inner) };
    }

    const VWeak<VTable, Dyn> &into_dyn() const
    {
        return *reinterpret_cast<const VWeak<VTable, Dyn> *>(this);
    }

    friend bool operator==(const VWeak &a, const VWeak &b) { return a.inner == b.inner; }
    friend bool operator!=(const VWeak &a, const VWeak &b) { return a.inner != b.inner; }
    const VTable *vtable() const { return inner ? inner->vtable : nullptr; }
};

template<typename VTable, typename MappedType>
class VRcMapped
{
    VRc<VTable, Dyn> parent_strong;
    MappedType *object;

    template<typename VTable_, typename MappedType_>
    friend class VWeakMapped;

public:
    /// Constructs a pointer to MappedType that shares ownership with parent_strong.
    template<typename X>
    explicit VRcMapped(VRc<VTable, X> parent_strong, MappedType *object)
        : parent_strong(parent_strong.into_dyn()), object(object)
    {
    }

    const MappedType *operator->() const { return object; }
    const MappedType &operator*() const { return *object; }
    MappedType *operator->() { return object; }
    MappedType &operator*() { return *object; }
};

template<typename VTable, typename MappedType>
class VWeakMapped
{
    VWeak<VTable, Dyn> parent_weak;
    MappedType *object = nullptr;

public:
    VWeakMapped(const VRcMapped<VTable, MappedType> &strong)
        : parent_weak(strong.parent_strong), object(strong.object)
    {
    }
    VWeakMapped() = default;

    std::optional<VRcMapped<VTable, MappedType>> lock() const
    {
        if (auto parent = parent_weak.lock()) {
            return VRcMapped<VTable, MappedType>(std::move(*parent), object);
        } else {
            return {};
        }
    }
};

} // namespace vtable
