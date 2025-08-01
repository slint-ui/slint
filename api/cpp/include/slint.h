// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint_internal.h"
#include "slint_platform_internal.h"
#include "slint_qt_internal.h"
#include "slint_window.h"
#include "slint_models.h"
#include "slint_item_tree.h"

#include <vector>
#include <chrono>
#include <span>
#include <concepts>

#ifndef SLINT_FEATURE_FREESTANDING
#    include <mutex>
#    include <condition_variable>
#endif

/// \rst
/// The :code:`slint` namespace is the primary entry point into the Slint C++ API.
/// All available types are in this namespace.
///
/// See the :doc:`Overview <../overview>` documentation for the C++ integration how
/// to load :code:`.slint` designs.
/// \endrst
namespace slint {

namespace private_api {

/// Convert a slint `{height: length, width: length, x: length, y: length}` to a Rect
inline cbindgen_private::Rect convert_anonymous_rect(std::tuple<float, float, float, float> tuple)
{
    // alphabetical order
    auto [h, w, x, y] = tuple;
    return cbindgen_private::Rect { .x = x, .y = y, .width = w, .height = h };
}

inline void dealloc(const ItemTreeVTable *vtable, uint8_t *ptr,
                    [[maybe_unused]] vtable::Layout layout)
{
    vtable::dealloc(vtable, ptr, layout);
}

template<typename T>
inline vtable::Layout drop_in_place(ItemTreeRef item_tree)
{
    return vtable::drop_in_place<ItemTreeVTable, T>(item_tree);
}

#if !defined(DOXYGEN)
#    if defined(_WIN32) || defined(_WIN64)
// On Windows cross-dll data relocations are not supported:
//     https://docs.microsoft.com/en-us/cpp/c-language/rules-and-limitations-for-dllimport-dllexport?view=msvc-160
// so we have a relocation to a function that returns the address we seek. That
// relocation will be resolved to the locally linked stub library, the implementation of
// which will be patched.
#        define SLINT_GET_ITEM_VTABLE(VTableName) slint::private_api::slint_get_##VTableName()
#    else
#        define SLINT_GET_ITEM_VTABLE(VTableName) (&slint::private_api::VTableName)
#    endif
#endif // !defined(DOXYGEN)

inline std::optional<cbindgen_private::ItemRc>
upgrade_item_weak(const cbindgen_private::ItemWeak &item_weak)
{
    if (auto item_tree_strong = item_weak.item_tree.lock()) {
        return { { *item_tree_strong, item_weak.index } };
    } else {
        return std::nullopt;
    }
}

inline void debug(const SharedString &str)
{
    cbindgen_private::slint_debug(&str);
}

} // namespace private_api

namespace cbindgen_private {
inline LayoutInfo LayoutInfo::merge(const LayoutInfo &other) const
{
    // Note: This "logic" is duplicated from LayoutInfo::merge in layout.rs.
    return LayoutInfo { std::min(max, other.max),
                        std::min(max_percent, other.max_percent),
                        std::max(min, other.min),
                        std::max(min_percent, other.min_percent),
                        std::max(preferred, other.preferred),
                        std::min(stretch, other.stretch) };
}
inline bool operator==(const EasingCurve &a, const EasingCurve &b)
{
    if (a.tag != b.tag) {
        return false;
    } else if (a.tag == EasingCurve::Tag::CubicBezier) {
        return std::equal(a.cubic_bezier._0, a.cubic_bezier._0 + 4, b.cubic_bezier._0);
    }
    return true;
}
}

namespace private_api {

inline static void register_item_tree(const vtable::VRc<ItemTreeVTable> *c,
                                      const std::optional<slint::Window> &maybe_window)
{
    const cbindgen_private::WindowAdapterRcOpaque *window_ptr =
            maybe_window.has_value() ? &maybe_window->window_handle().handle() : nullptr;
    cbindgen_private::slint_register_item_tree(c, window_ptr);
}

inline SharedVector<float> solve_box_layout(const cbindgen_private::BoxLayoutData &data,
                                            cbindgen_private::Slice<int> repeater_indexes)
{
    SharedVector<float> result;
    cbindgen_private::Slice<uint32_t> ri { reinterpret_cast<uint32_t *>(repeater_indexes.ptr),
                                           repeater_indexes.len };
    cbindgen_private::slint_solve_box_layout(&data, ri, &result);
    return result;
}

inline SharedVector<float> solve_grid_layout(const cbindgen_private::GridLayoutData &data)
{
    SharedVector<float> result;
    cbindgen_private::slint_solve_grid_layout(&data, &result);
    return result;
}

inline cbindgen_private::LayoutInfo
grid_layout_info(cbindgen_private::Slice<cbindgen_private::GridLayoutCellData> cells, float spacing,
                 const cbindgen_private::Padding &padding)
{
    return cbindgen_private::slint_grid_layout_info(cells, spacing, &padding);
}

inline cbindgen_private::LayoutInfo
box_layout_info(cbindgen_private::Slice<cbindgen_private::BoxLayoutCellData> cells, float spacing,
                const cbindgen_private::Padding &padding,
                cbindgen_private::LayoutAlignment alignment)
{
    return cbindgen_private::slint_box_layout_info(cells, spacing, &padding, alignment);
}

inline cbindgen_private::LayoutInfo
box_layout_info_ortho(cbindgen_private::Slice<cbindgen_private::BoxLayoutCellData> cells,
                      const cbindgen_private::Padding &padding)
{
    return cbindgen_private::slint_box_layout_info_ortho(cells, &padding);
}

/// Access the layout cache of an item within a repeater
inline float layout_cache_access(const SharedVector<float> &cache, int offset, int repeater_index)
{
    size_t idx = size_t(cache[offset]) + repeater_index * 2;
    return idx < cache.size() ? cache[idx] : 0;
}

template<typename VT, typename ItemType>
inline cbindgen_private::LayoutInfo
item_layout_info(VT *itemvtable, ItemType *item_ptr, cbindgen_private::Orientation orientation,
                 WindowAdapterRc *window_adapter, const ItemTreeRc &component_rc,
                 uint32_t item_index)
{
    cbindgen_private::ItemRc item_rc { component_rc, item_index };
    return itemvtable->layout_info({ itemvtable, item_ptr }, orientation, window_adapter, &item_rc);
}
} // namespace private_api

namespace private_api {

template<typename T>
union MaybeUninitialized {
    T value;
    ~MaybeUninitialized() { }
    MaybeUninitialized() { }
    T take()
    {
        T result = std::move(value);
        value.~T();
        return result;
    }
};

inline void setup_popup_menu_from_menu_item_tree(
        const ItemTreeRc &menu_item_tree,
        Property<std::shared_ptr<Model<cbindgen_private::MenuEntry>>> &entries,
        Callback<std::shared_ptr<Model<cbindgen_private::MenuEntry>>(cbindgen_private::MenuEntry)>
                &sub_menu,
        Callback<void(cbindgen_private::MenuEntry)> &activated)
{
    using cbindgen_private::MenuEntry;
    using cbindgen_private::MenuVTable;
    MaybeUninitialized<vtable::VRc<MenuVTable>> maybe;
    cbindgen_private::slint_menus_create_wrapper(&menu_item_tree, &maybe.value);
    auto shared = maybe.take();
    entries.set_binding([shared] {
        SharedVector<MenuEntry> entries_sv;
        shared.vtable()->sub_menu(shared.borrow(), nullptr, &entries_sv);
        std::vector<MenuEntry> entries_vec(entries_sv.begin(), entries_sv.end());
        return std::make_shared<VectorModel<MenuEntry>>(std::move(entries_vec));
    });
    sub_menu.set_handler([shared](const auto &entry) {
        SharedVector<MenuEntry> entries_sv;
        shared.vtable()->sub_menu(shared.borrow(), &entry, &entries_sv);
        std::vector<MenuEntry> entries_vec(entries_sv.begin(), entries_sv.end());
        return std::make_shared<VectorModel<MenuEntry>>(std::move(entries_vec));
    });
    activated.set_handler(
            [shared](const auto &entry) { shared.vtable()->activate(shared.borrow(), &entry); });
}

inline SharedString translate(const SharedString &original, const SharedString &context,
                              const SharedString &domain,
                              cbindgen_private::Slice<SharedString> arguments, int n,
                              const SharedString &plural)
{
    SharedString result = original;
    cbindgen_private::slint_translate(&result, &context, &domain, arguments, n, &plural);
    return result;
}

inline SharedString translate_from_bundle(std::span<const char8_t *const> strs,
                                          cbindgen_private::Slice<SharedString> arguments)
{
    SharedString result;
    cbindgen_private::slint_translate_from_bundle(
            cbindgen_private::Slice<const char *>(
                    const_cast<char const **>(reinterpret_cast<char const *const *>(strs.data())),
                    strs.size()),
            arguments, &result);
    return result;
}
inline SharedString
translate_from_bundle_with_plural(std::span<const char8_t *const> strs,
                                  std::span<const uint32_t> indices,
                                  std::span<uintptr_t (*const)(int32_t)> plural_rules,
                                  cbindgen_private::Slice<SharedString> arguments, int n)
{
    SharedString result;
    cbindgen_private::Slice<const char *> strs_slice(
            const_cast<char const **>(reinterpret_cast<char const *const *>(strs.data())),
            strs.size());
    cbindgen_private::Slice<uint32_t> indices_slice(
            const_cast<uint32_t *>(reinterpret_cast<const uint32_t *>(indices.data())),
            indices.size());
    cbindgen_private::Slice<uintptr_t (*)(int32_t)> plural_rules_slice(
            const_cast<uintptr_t (**)(int32_t)>(
                    reinterpret_cast<uintptr_t (*const *)(int32_t)>(plural_rules.data())),
            plural_rules.size());
    cbindgen_private::slint_translate_from_bundle_with_plural(
            strs_slice, indices_slice, plural_rules_slice, arguments, n, &result);
    return result;
}

} // namespace private_api

#ifdef SLINT_FEATURE_GETTEXT
/// Forces all the strings that are translated with `@tr(...)` to be re-evaluated.
/// This is useful if the language is changed at runtime.
/// The function is only available when Slint is compiled with `SLINT_FEATURE_GETTEXT`.
///
/// Example
/// ```cpp
///     my_ui->global<LanguageSettings>().on_french_selected([] {
///        setenv("LANGUAGE", langs[l], true);
///        slint::update_all_translations();
///    });
/// ```
inline void update_all_translations()
{
    cbindgen_private::slint_translations_mark_dirty();
}
#endif

/// Select the current translation language when using bundled translations.
/// This function requires that the application's `.slint` file was compiled with bundled
/// translations. It must be called after creating the first component.
///
/// The language string is the locale, which matches the name of the folder that contains the
/// `LC_MESSAGES` folder. An empty string or `"en"` will select the default language.
///
/// Returns true if the language was selected; false if the language was not found in the list of
/// bundled translations.
inline bool select_bundled_translation(std::string_view language)
{
    return cbindgen_private::slint_translate_select_bundled_translation(
            slint::private_api::string_to_slice(language));
}

#if !defined(DOXYGEN)
cbindgen_private::Flickable::Flickable()
{
    slint_flickable_data_init(&data);
}
cbindgen_private::Flickable::~Flickable()
{
    slint_flickable_data_free(&data);
}

cbindgen_private::NativeStyleMetrics::NativeStyleMetrics(void *)
{
    slint_native_style_metrics_init(this);
}

cbindgen_private::NativeStyleMetrics::~NativeStyleMetrics()
{
    slint_native_style_metrics_deinit(this);
}

cbindgen_private::NativePalette::NativePalette(void *)
{
    slint_native_palette_init(this);
}

cbindgen_private::NativePalette::~NativePalette()
{
    slint_native_palette_deinit(this);
}
#endif // !defined(DOXYGEN)

namespace private_api {
// Was used in Slint <= 1.1.0 to have an error message in case of mismatch
template<int Major, int Minor, int Patch>
struct [[deprecated]] VersionCheckHelper
{
};
}

/// Enum for the event loop mode parameter of the slint::run_event_loop() function.
/// It is used to determine when the event loop quits.
enum class EventLoopMode {
    /// The event loop will quit when the last window is closed
    /// or when slint::quit_event_loop() is called.
    QuitOnLastWindowClosed,

    /// The event loop will keep running until slint::quit_event_loop() is called,
    /// even when all windows are closed.
    RunUntilQuit
};

/// Enters the main event loop. This is necessary in order to receive
/// events from the windowing system in order to render to the screen
/// and react to user input.
///
/// The mode parameter determines the behavior of the event loop when all windows are closed.
/// By default, it is set to QuitOnLastWindowClose, which means the event loop will
/// quit when the last window is closed.
inline void run_event_loop(EventLoopMode mode = EventLoopMode::QuitOnLastWindowClosed)
{
    private_api::assert_main_thread();
    cbindgen_private::slint_run_event_loop(mode == EventLoopMode::QuitOnLastWindowClosed);
}

/// Schedules the main event loop for termination. This function is meant
/// to be called from callbacks triggered by the UI. After calling the function,
/// it will return immediately and once control is passed back to the event loop,
/// the initial call to slint::run_event_loop() will return.
inline void quit_event_loop()
{
    cbindgen_private::slint_quit_event_loop();
}

/// Adds the specified functor to an internal queue, notifies the event loop to wake up.
/// Once woken up, any queued up functors will be invoked.
/// This function is thread-safe and can be called from any thread, including the one
/// running the event loop. The provided functors will only be invoked from the thread
/// that started the event loop.
///
/// You can use this to set properties or use any other Slint APIs from other threads,
/// by collecting the code in a functor and queuing it up for invocation within the event loop.
///
/// The following example assumes that a status message received from a network thread is
/// shown in the UI:
///
/// ```
/// #include "my_application_ui.h"
/// #include <thread>
///
/// int main(int argc, char **argv)
/// {
///     auto ui = NetworkStatusUI::create();
///     ui->set_status_label("Pending");
///
///     slint::ComponentWeakHandle<NetworkStatusUI> weak_ui_handle(ui);
///     std::thread network_thread([=]{
///         std::string message = read_message_blocking_from_network();
///         slint::invoke_from_event_loop([&]() {
///             if (auto ui = weak_ui_handle.lock()) {
///                 ui->set_status_label(message);
///             }
///         });
///     });
///     ...
///     ui->run();
///     ...
/// }
/// ```
///
/// See also blocking_invoke_from_event_loop() for a blocking version of this function
template<std::invocable Functor>
void invoke_from_event_loop(Functor f)
{
    cbindgen_private::slint_post_event(
            [](void *data) { (*reinterpret_cast<Functor *>(data))(); }, new Functor(std::move(f)),
            [](void *data) { delete reinterpret_cast<Functor *>(data); });
}

#if !defined(SLINT_FEATURE_FREESTANDING) || defined(DOXYGEN)

/// Blocking version of invoke_from_event_loop()
///
/// Just like invoke_from_event_loop(), this will run the specified functor from the thread running
/// the slint event loop. But it will block until the execution of the functor is finished,
/// and return that value.
///
/// This function must be called from a different thread than the thread that runs the event loop
/// otherwise it will result in a deadlock. Calling this function if the event loop is not running
/// will also block forever or until the event loop is started in another thread.
///
/// The following example is reading the message property from a thread
///
/// ```
/// #include "my_application_ui.h"
/// #include <thread>
///
/// int main(int argc, char **argv)
/// {
///     auto ui = MyApplicationUI::create();
///     ui->set_status_label("Pending");
///
///     std::thread worker_thread([ui]{
///         while (...) {
///             auto message = slint::blocking_invoke_from_event_loop([ui]() {
///                return ui->get_message();
///             }
///             do_something(message);
///             ...
///         });
///     });
///     ...
///     ui->run();
///     ...
/// }
/// ```
template<std::invocable Functor>
auto blocking_invoke_from_event_loop(Functor f) -> std::invoke_result_t<Functor>
{
    std::optional<std::invoke_result_t<Functor>> result;
    std::mutex mtx;
    std::condition_variable cv;
    invoke_from_event_loop([&] {
        auto r = f();
        std::unique_lock lock(mtx);
        result = std::move(r);
        cv.notify_one();
    });
    std::unique_lock lock(mtx);
    cv.wait(lock, [&] { return result.has_value(); });
    return std::move(*result);
}

#    if !defined(DOXYGEN) // Doxygen doesn't see this as an overload of the previous one
// clang-format off
template<std::invocable Functor>
    requires(std::is_void_v<std::invoke_result_t<Functor>>)
void blocking_invoke_from_event_loop(Functor f)
// clang-format on
{
    std::mutex mtx;
    std::condition_variable cv;
    bool ok = false;
    invoke_from_event_loop([&] {
        f();
        std::unique_lock lock(mtx);
        ok = true;
        cv.notify_one();
    });
    std::unique_lock lock(mtx);
    cv.wait(lock, [&] { return ok; });
}
#    endif
#endif

/// Sets the application id for use on Wayland or X11 with
/// [xdg](https://specifications.freedesktop.org/desktop-entry-spec/latest/) compliant window
/// managers. This must be set before the window is shown.
inline void set_xdg_app_id(std::string_view xdg_app_id)
{
    private_api::assert_main_thread();
    SharedString s = xdg_app_id;
    cbindgen_private::slint_set_xdg_app_id(&s);
}

} // namespace slint
