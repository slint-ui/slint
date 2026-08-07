<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Vulkan Texture Import Example

This example draws with plain Vulkan into a texture and shows that texture in a Slint scene:

1. A ray marched cube is rendered with Vulkan into a texture.
2. The texture is imported into a `slint::Image` and set on an `Image` element.
3. Slint renders the scene, with the texture shown in the `Image`.

It's the same effect as the `opengl_texture` and `wgpu_texture` examples next door, so the three
can be compared side by side.

## Where the Vulkan handles come from

The example never creates a Vulkan instance, device or queue of its own. Slint renders with Skia
on top of wgpu here, wgpu is asked for its Vulkan backend, and the example borrows the handles
wgpu already made:

```rust
let hal_device = wgpu_device.as_hal::<wgpu::wgc::api::Vulkan>()?;
let hal_queue = wgpu_queue.as_hal::<wgpu::wgc::api::Vulkan>()?;
(hal_device.raw_device().clone(), hal_queue.as_raw(), hal_device.queue_family_index())
```

`wgpu_device` and `wgpu_queue` come from `slint::GraphicsAPI::WGPU30`, which
`slint::Window::set_rendering_notifier()` hands to its callback. Sharing one device and one queue
is what lets Slint import the result without copying it, and it's why the drawing can be ordered
against Slint's own rendering with nothing but submission order.

The render target is allocated by wgpu rather than by this example, because wgpu owns the memory
and the lifetime and because `slint::Image::try_from` takes a `wgpu::Texture`. Only the `VkImage`
underneath it is borrowed, via `texture.as_hal::<Vulkan>()?.raw_handle()`.

## Handing the texture back and forth

Two things need care when raw Vulkan and wgpu share a texture.

**wgpu can't see the drawing.** wgpu tracks the state of every resource it knows about, and this
example's command buffer is invisible to that tracking. Before submitting, the example tells wgpu
what is about to happen:

```rust
encoder.transition_resources(std::iter::empty(), std::iter::once(wgpu::TextureTransition {
    texture: &target.texture,
    selector: None,
    state: wgpu::TextureUses::COLOR_TARGET,
}));
```

Without it wgpu still believes the texture is untouched, and the barrier Slint later emits to hand
the texture to Skia for sampling names a source scope that doesn't cover these writes. The Vulkan
synchronization validation layer reports that as a `WRITE_AFTER_WRITE` hazard.

**The image layout has to match on both sides.** The render pass hands the image back in
`COLOR_ATTACHMENT_OPTIMAL`, matching the `COLOR_TARGET` state wgpu was just told about. It starts
from `UNDEFINED`, because the whole image is redrawn every frame and that saves tracking whatever
layout Skia and wgpu left it in.

The render pass also spells out its dependency on `VK_SUBPASS_EXTERNAL`. Both this example's
previous frame and Skia's sampling of it ran on the same queue, and the implicit dependency starts
at `TOP_OF_PIPE`, which would order the drawing after neither.

## Running it

The example asks for `wgpu::Backends::VULKAN`, so no backend selection is needed:

```sh
cargo run -p vulkan_texture
```

On macOS and iOS Vulkan runs on MoltenVK, through wgpu's `vulkan-portability` feature, which the
`slint/wgpu-30-vulkan-portability` feature in `Cargo.toml` turns on. It needs the
[Vulkan SDK](https://vulkan.lunarg.com/) installed, and the loader has to be findable: `ash` opens
a bare `libvulkan.dylib`, which dyld doesn't look for in `/usr/local/lib` on its own.

```sh
env DYLD_LIBRARY_PATH=/usr/local/lib cargo run -p vulkan_texture
```

Use `env`, not an exported variable: macOS strips `DYLD_*` from the environment of protected
binaries such as the shell itself.

## Checking it against the validation layers

```sh
env DYLD_LIBRARY_PATH=/usr/local/lib \
    VK_LOADER_LAYERS_ENABLE=VK_LAYER_KHRONOS_validation \
    VK_LAYER_SETTINGS_PATH=vk_layer_settings.txt \
    cargo run -p vulkan_texture
```

with a `vk_layer_settings.txt` of:

```
khronos_validation.validate_sync = true
khronos_validation.debug_action = VK_DBG_LAYER_ACTION_LOG_MSG
khronos_validation.log_filename = stdout
khronos_validation.report_flags = error,warn,perf
```

Routing the layer's output to `stdout` matters: the example installs no `log` logger, so nothing
reaching wgpu's own debug messenger would be printed.

One error is expected and comes from wgpu itself, not from this example:
`VUID-vkAcquireNextImageKHR-fence-10066`, once per frame. wgpu 30.0.0 passes a real fence to
`vkAcquireNextImageKHR` but only waits on and resets it on Windows. It's fixed on wgpu trunk.

## Regenerating the shaders

`shader.vert.spv` and `shader.frag.spv` are checked in so that building the example doesn't need a
shader compiler. After editing the GLSL, rebuild them with `glslangValidator` from the Vulkan SDK:

```sh
glslangValidator -V shader.vert -o shader.vert.spv
glslangValidator -V shader.frag -o shader.frag.spv
```
