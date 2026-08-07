// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include "slint.h"

#if !defined(SLINT_FEATURE_UNSTABLE_WGPU_30) && !defined(DOXYGEN)
#    error                                                                                         \
            "slint-vulkan.h needs Slint built with a Skia renderer (SLINT_FEATURE_RENDERER_SKIA)"
#endif

#if !__has_include(<vulkan/vulkan.h>) && !defined(DOXYGEN)
#    error                                                                                         \
            "slint-vulkan.h needs the Vulkan headers (install the Vulkan SDK, or libvulkan-dev)"
#endif

#include <vulkan/vulkan.h>

#include "private/slint_platform_internal.h"

/// Use the types in this namespace to render with Vulkan into what Slint shows.
///
/// Slint renders through wgpu, and asks it for its Vulkan backend. Everything here refers to the
/// objects wgpu created: an application draws with the same device and submits on the same queue,
/// which is what lets Slint show the result without copying it, and what orders the drawing
/// against Slint's own without any semaphore of the application's.
///
/// *Note*: These types are behind a feature flag and may be removed or changed in future minor
///         releases, as new major WGPU releases become available.
namespace slint::vulkan {

/// The Vulkan objects the renderer is rendering with.
///
/// An instance of this is handed to the callback registered with
/// [set_rendering_notifier()](slint::vulkan::set_rendering_notifier()), and refers to objects
/// owned by the renderer. They stay valid from `RenderingState::RenderingSetup` until
/// `RenderingState::RenderingTeardown`; the `Api` object itself only for the duration of the call.
class Api
{
public:
    /// \private
    explicit Api(const cbindgen_private::VulkanApi &api) : inner(api) { }

    /// Returns the instance the renderer created.
    VkInstance instance() const { return reinterpret_cast<VkInstance>(inner.instance); }

    /// Returns the physical device the renderer picked.
    VkPhysicalDevice physical_device() const
    {
        return reinterpret_cast<VkPhysicalDevice>(inner.physical_device);
    }

    /// Returns the device the renderer created.
    ///
    /// Allocate what you hand back to Slint from this device; the renderer cannot use resources
    /// belonging to another one.
    VkDevice device() const { return reinterpret_cast<VkDevice>(inner.device); }

    /// Returns the queue the renderer submits on.
    ///
    /// Submitting your own command buffers on this same queue is what orders them against the
    /// renderer's work, without needing a semaphore of your own.
    VkQueue queue() const { return reinterpret_cast<VkQueue>(inner.queue); }

    /// Returns the index of the queue family queue() belongs to.
    uint32_t queue_family_index() const { return inner.queue_family_index; }

    /// Returns `vkGetInstanceProcAddr` of the loader the renderer is using.
    ///
    /// Resolve entry points through this to be sure of talking to the same driver as the
    /// renderer, rather than to whichever loader the application itself linked against.
    PFN_vkGetInstanceProcAddr get_instance_proc_addr() const
    {
        return reinterpret_cast<PFN_vkGetInstanceProcAddr>(inner.get_instance_proc_addr);
    }

private:
    const cbindgen_private::VulkanApi &inner;
};

/// Registers a callback that's invoked during the different phases of rendering, with the Vulkan
/// objects the renderer is using.
///
/// The callback is called with a `slint::RenderingState` and a pointer to an Api. That pointer is
/// null whenever the renderer isn't running on Vulkan, which is worth reporting rather than
/// ignoring: it means the application's own rendering can't take place.
///
/// On success the returned std::optional has no value. On error it holds the reason.
template<std::invocable<RenderingState, const Api *> F>
inline std::optional<SetRenderingNotifierError> set_rendering_notifier(const Window &window,
                                                                       F &&callback)
{
    private_api::assert_main_thread();

    using Callback = std::decay_t<F>;

    auto actual_cb = [](RenderingState state, const cbindgen_private::VulkanApi *raw_api,
                        void *user_data) {
        auto &f = *reinterpret_cast<Callback *>(user_data);
        if (raw_api) {
            Api api(*raw_api);
            f(state, &api);
        } else {
            f(state, nullptr);
        }
    };

    SetRenderingNotifierError err;
    if (cbindgen_private::slint_windowrc_set_vulkan_rendering_notifier(
                &window.window_handle().handle(), actual_cb,
                [](void *user_data) { delete reinterpret_cast<Callback *>(user_data); },
                new Callback(std::forward<F>(callback)), &err)) {
        return {};
    }
    return err;
}

}
