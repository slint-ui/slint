# Slint on Micro-Controllers (MCU)

This document explain how to use slint to develop a UI on a MCU.

## Install toolchain / hal

Each MCU or board needs the proper toolchain for cross compilation,
has its own hal crate (Hardware Abstraction Layer) and drivers, and other tools to flash and debug the device.

This is out of scope for this document. You can check the [Rust Embedded Book](https://docs.rust-embedded.org/book/)
or other resources specific to your device that will guide you to get a "hello world" working on your device.

You will need nightly Rust, since stable Rust unfortunately doesn't provide a way to use a global allocator in a `#![no_std]` project.
(until [#51540](https://github.com/rust-lang/rust/issues/51540) or [#66741](https://github.com/rust-lang/rust/issues/66741) is stabilized)

## Set the feature flags

A typical line in Cargo.toml looks like that:

```toml
[dependencies]
slint = { version = "0.2.6", default-features = false, features = ["compat-0.3.0", "unsafe-single-threaded", "libm", "renderer-winit-software"] }
# ... other stuf
```

Slint uses the standard library by default, so we need to disable the default features.
Then you need the `compat-0.3.0` feature ([see why in this blog post](https://slint-ui.com/blog/rust-adding-default-cargo-feature.html))

As we don't have `std`, you will also need to enable the `unsafe-single-threaded` feature: Slint can't use `thread_local!` and will use unsafe static instead.
By setting this feature, you promise not to use Slint API from a different thread or interrupt handler.

You will also need the `libm` feature to for the math operation that would otherwise be taken care by the std lib.

And the additional feature you need is `renderer-winit-software` to enable the software renderer we will need to render a Slint scene on MCU.

## `build.rs`

When targeting MCU, you will need a build script to compile the `.slint` files using the `slint-build` crate.
You will have to use the `slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer` configuration option to tell
the slint compiler to embed the images and font in the binary in the proper format.

```rust,no_run
fn main() {
    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new()
            .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer),
    ).unwrap();
}
```

## The `Platform` trait

The idea is to call `[slint::platform::set_platform]` before constructing your Slint application.

The [`Platform`] trait has two main responsibilities:
 1. Give a window that will be used when creating your component with `new()`
 2. Be a source of time. Since on bare metal, we don't have [`std::time::Instant`] as a
    source of time, so you need to provide the time from some time source that will likely
    be provided from the hal crate of your device.

Optionally, you can also use the Platform trait to run the event loop.

A typical platform looks like this:

```rust,no_run
#![no_std]
extern crate alloc;
use alloc::{rc::Rc, boxed::Box};
# mod hal { pub struct Timer(); impl Timer { pub fn get_time(&self) -> u64 { todo!() } } }
use slint::platform::{Platform, software_renderer::MinimalSoftwareWindow};

# slint::slint!{ export MyUI := Window {} } /*
slint::include_modules!();
# */

struct MyPlatform {
    window: Rc<MinimalSoftwareWindow<2>>,
    // optional: some timer device from your device's HAL crate
    timer: hal::Timer,
    // ... maybe more devices
}

impl Platform for MyPlatform {
    fn create_window_adapter(&self) -> Rc<dyn slint::platform::WindowAdapter> {
        // Since on MCU, there can be only one window, just return a clone of self.window.
        // We'll also use the same window in the event loop
        self.window.clone()
    }
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_micros(self.timer.get_time())
    }
    // optional: You can put the event loop there, or in the main function, see later
    fn run_event_loop(&self) {
        todo!();
    }
}

// #[hal::entry]
fn main() {
    // init the allocator, and other devices stuff
    //...

    // init a window (we'll need it later)
    let window = MinimalSoftwareWindow::new();
    slint::platform::set_platform(Box::new(MyPlatform {
        window: window.clone(),
        timer: hal::Timer(/*...*/),
        //...
    }))
    .unwrap();

    // setup the UI
    let ui = MyUI::new();
    // ... setup callback and properties on `ui` ...

    // Make sure the window cover our all screen
    window.set_size(slint::PhysicalSize::new(320, 240));

    // ... start event loop (see later) ...
}
```

## The event loop

Once you have initialized your Platform, you can start the event loop.
You've got two choices:
 1. Implement [`slint::platform::Platform::run_event_loop`]. In this case, you can start
    the event loop in a similar way than on desktop platform using the [`run()`](slint::ComponentHandle::run) function
    of your component, or use [`slint::run_event_loop()`].
 2. Use a `loop { ... }` directly in your main function.

 The second option might be more convenient on MCUs because you can initialize all the devices in your main function
 and you can access them in there without moving them in your Platform implementation.
 In our examples, we use the first option so we can use a different Platform with the same code to
 run on different devices.

A typical event-loop looks like this:

```rust,no_run
use slint::platform::{software_renderer::MinimalSoftwareWindow};
let window = MinimalSoftwareWindow::<0>::new();
# fn check_for_touch_event() -> Option<slint::WindowEvent> { todo!() }
# mod hal { pub fn wfi() {} }
//...
loop {
    // Let slint run the timer hooks and update animations
    slint::platform::update_timers_and_animations();

    // Check the touch screen or input device  using your driver
    if let Some(event) = check_for_touch_event(/*...*/) {
        // convert the event from the driver into a `slint::WindowEvent`
        // and pass it to the window
        window.dispatch_event(event);
    }

    // Draw the scene if something needs to be drawn
    window.draw_if_needed(|renderer| {
        // see next section
        todo!()
    });

    // ... maybe some more application logic ...

    // Put the MCU to sleep
    if !window.has_active_animations() {
        if let Some(duration) = slint::platform::duration_until_next_timer_update() {
            // ... schedule an interrupt in `duration` ...
        }
        hal::wfi(); // Wait for interupt
    }
}

```

## The renderer

On MCU, we currently only support software rendering. In the previous example, we've instantiated a
[`slint::platform::software_renderer::MinimalSoftwareWindow`]. This will give us an instance of the
[`slint::platform::software_renderer::SoftwareRenderer`] through the
[`draw_if_needed()`](MinimalSoftwareWindow::draw_if_needed) function.

There are two ways to render, depending on the kind of screen and the amount of RAM.

If you have enough RAM to hold one, or even two, frame buffer, you can use the
[`SoftwareRenderer::render()`] function.
Otherwise, if you can't hold a frame buffer in memory, you can render line by line and send these
line of pixel to the screen (typically via SPI). In that case, you would use the
[`SoftwareRenderer::render_by_line()`] function.

Either way, you would render to a buffer (a full, or just a line), which is a slice of pixel.
So a slice of something that implement the [`slint::platform::software_renderer::TargetPixel`] trait.
By default, this trait is implemented for [`slint::Rgb8Pixel`] and [`slint::platform::software_renderer::Rgb565Pixel`].

### Rendering in a buffer

In this example, we'll use double buffering and swap between the buffer.

```rust,no_run
use slint::platform::software_renderer::Rgb565Pixel;
# fn is_swap_pending()->bool {false} fn swap_buffers() {}

// Note that we use `2` as the const generic parameter which is our buffer count,
// since we have two buffer, we always need to refresh what changed in the two
// previous frames
let window = slint::platform::software_renderer::MinimalSoftwareWindow::<2>::new();

const DISPLAY_WIDTH: usize = 320;
const DISPLAY_HEIGHT: usize = 240;
let mut buffer1 = [Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];
let mut buffer2 = [Rgb565Pixel(0); DISPLAY_WIDTH * DISPLAY_HEIGHT];

// ... configure the screen driver to use buffer1 or buffer2 ...

// ... rest of initialization ...

let mut currently_displayed_buffer : &mut [_] = &mut buffer1;
let mut work_buffer : &mut [_] = &mut buffer2;

loop {
    // ...
    // Draw the scene if something needs to be drawn
    window.draw_if_needed(|renderer| {
        // The screen driver might be taking some time to do the swap. We need to wait until
        // work_buffer is ready to be written in
        while is_swap_pending() {}

        // Do the rendering!
        renderer.render(work_buffer, DISPLAY_WIDTH);

        // tell the screen driver to display the other buffer.
        swap_buffers();

        // Swap the buffer references for our next iteration
        // (this just swap the reference, not the actual data)
        core::mem::swap::<&mut [_]>(&mut work_buffer, &mut currently_displayed_buffer);
    });
    // ...
}

```

### Render line by line

The line by line provider works by implementing the [`LineBufferProvider`] trait.

This example use a screen driver that implements the [`embedded_graphics`](https://lib.rs/embedded-graphics) traits
by using the [DrawTarget::fill_contiguous](https://docs.rs/embedded-graphics/0.7.1/embedded_graphics/draw_target/trait.DrawTarget.html)
function

```rust,no_run
use embedded_graphics_core::{prelude::*, primitives::Rectangle, pixelcolor::raw::RawU16};

# mod embedded_graphics_core {
#  pub mod prelude {
#    pub struct Point; impl Point { pub fn new(_:i32, _:i32) -> Self {todo!()} }
#    pub struct Size; impl Size { pub fn new(_:i32, _:i32) -> Self {todo!()} }
#    pub trait DrawTarget { type Color; fn fill_contiguous(&mut self, _: &super::primitives::Rectangle, _: impl IntoIterator<Item = Self::Color>) -> Result<(), ()> {Ok(())} }
#  }
#  pub mod primitives { pub struct Rectangle; impl Rectangle { pub fn new(_: super::prelude::Point, _: super::prelude::Size) -> Self { todo!() } } }
#  pub mod pixelcolor {
#    pub struct Rgb565;
#    pub mod raw { pub struct RawU16(); impl RawU16 { pub fn new(_:u16) -> Self {todo!()} } impl From<RawU16> for super::Rgb565 { fn from(_: RawU16) -> Self {todo!()} } }
#  }
# }
# mod hal { pub struct Display; impl Display { pub fn new()-> Self {todo!()} } }
# impl DrawTarget for hal::Display{ type Color = embedded_graphics_core::pixelcolor::Rgb565; }

struct DisplayWrapper<'a, T>{
    display: &'a mut T,
    line_buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel],
};
impl<T: DrawTarget<Color = embedded_graphics_core::pixelcolor::Rgb565>>
    slint::platform::software_renderer::LineBufferProvider for DisplayWrapper<'_, T>
{
    type TargetPixel = slint::platform::software_renderer::Rgb565Pixel;
    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [Self::TargetPixel]),
    ) {
        // Render into the line
        render_fn(&mut self.line_buffer[range.clone()]);

        // Send the line to the screen using DrawTarget::fill_contiguous
        self.display.fill_contiguous(
            &Rectangle::new(Point::new(range.start as _, line as _), Size::new(range.len() as _, 1)),
            self.line_buffer[range.clone()].iter().map(|p| RawU16::new(p.0).into())
        ).map_err(drop).unwrap();
    }
}

// Note that we use `1` as the const generic parameter which is our buffer count.
// The buffer is not in our RAM, but actually within the display itself.
// We just need to re-render what changed in the last frame.
let window = slint::platform::software_renderer::MinimalSoftwareWindow::<1>::new();

const DISPLAY_WIDTH: usize = 320;
let mut line_buffer = [slint::platform::software_renderer::Rgb565Pixel(0); DISPLAY_WIDTH];

let mut display = hal::Display::new(/*...*/);

// ... rest of initialization ...

loop {
    // ...
    window.draw_if_needed(|renderer| {
        renderer.render_by_line(DisplayWrapper{
            display: &mut display,
            line_buffer: &mut line_buffer
        });
    });
    // ...
}

```

There might be faster way to do that than using the synchronous DrawTarget::fill_contiguous
function to do that.
For example, some device might be able to send the line to the display asynchronously using
DMA. In that case, we'd have two line buffer. One working line, and one which is being send
to the screen, in parallel.

## Our supported boards

Our example use a support crate containing an implementation of the [`Platform`] trait
for the device we tested.

You can also make use of that crate, but you will need to use `git="..."` in your Cargo.toml

<https://github.com/slint-ui/slint/tree/master/examples/mcu-board-support>
