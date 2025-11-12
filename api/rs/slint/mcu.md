<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Slint on Microcontrollers

![](https://slint.dev/blog/porting-slint-to-microcontrollers/rp-pico_and_screen.jpg)

The following sections explain how to use Slint to develop a UI on a Microcontroller (MCU) in a bare metal environment.

## Prerequisites

Writing an application in Rust that runs on a MCU requires several prerequisites:

* Install a Rust toolchain to cross-compile to the target architecture.
* Locate and select the correct Hardware Abstraction Layer (HAL) crates and drivers, and depend on them in your `Cargo.toml`.
* Install tools for flashing and debugging your code on the device.

We recommend reading the [Rust Embedded Book](https://docs.rust-embedded.org/book/),
and the curated list of [Awesome Embedded Rust](https://github.com/rust-embedded/awesome-embedded-rust) for a wide range of
crates, tools, and training materials. These resources should guide you through the initial setup. Many include a "hello world" example
to get started with your device.

Slint requires a global memory allocator in a bare metal environment with `#![no_std]`.

The following sections assume that your setup is complete and you have a non-graphical skeleton Rust program running on your MCU.

## Changes to `Cargo.toml`

Start by adding a dependency to the `slint` and the `slint-build` crates to your `Cargo.toml` using the `cargo` command:

Start with the `slint` crate like this:

```sh
cargo add slint@1.14 --no-default-features --features "compat-1-2 unsafe-single-threaded libm renderer-software"
```

The default features of the `slint` crate are tailored towards hosted environments and includes the "std" feature. In bare metal environments,
you need to disable the default features.

In the snippet above, three features are selected:

 * `compat-1-2`: We select this feature when disabling the default features. For a detailed explanation see our blog post ["Adding default cargo features without breaking Semantic Versioning"](https://slint.dev/blog/rust-adding-default-cargo-feature.html).
 * `unsafe-single-threaded`: Slint internally uses Rust's [`thread_local!`](https://doc.rust-lang.org/std/macro.thread_local.html) macro to store global data.
   This macro is only available in the Rust Standard Library (std), but not in bare metal environments. As a fallback, the `unsafe-single-threaded`
   feature changes Slint to use unsafe static for storage. This way, you guarantee to use Slint API only from a single thread, and not from interrupt handlers.
 * `libm`: We select this feature to enable the use of the [libm](https://crates.io/crates/libm) crate to provide traits and functions for floating point arithmetic.
   They're typically provided by the Rust Standard Library (std), but that's not available in bare metal environments.
 * `renderer-software`: We select this feature to use Slint's built-in software renderer.

It might be necessary to enable the [Feature resolver version 2](https://doc.rust-lang.org/cargo/reference/features.html#feature-resolver-version-2)
in your Cargo.toml if you notice that your dependencies are attempting to build with `std` support even when disabled.
This is the default when using the Rust 2021 Edition, but not if you use a workspace.

Then add the `slint-build` crate as a build dependency:

```sh
cargo add --build slint-build@1.14
```

For reference: These are the relevant parts of your `Cargo.toml` file,
ready to use Slint:

```toml
[package]
## ...
## Edition 2021 or later enables the feature resolver version 2.
edition = "2021"

[dependencies]
## ... your other dependencies

[dependencies.slint]
version = "1.14"
default-features = false
features = ["compat-1-2", "unsafe-single-threaded", "libm", "renderer-software"]
[build-dependencies]
slint-build = "1.14"
```

## Changes to `build.rs`

Next, write a build script to compile the `.slint` files to Rust code for embedding into the program binary, using the `slint-build` crate:

```rust,no_run
fn main() {
    slint_build::compile_with_config(
        "ui/main.slint",
        slint_build::CompilerConfiguration::new()
            .embed_resources(slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer),
    ).unwrap();
}
```

Use the `slint_build::EmbedResourcesKind::EmbedForSoftwareRenderer` configuration option to tell the Slint compiler to embed the images and fonts in the binary
in a format that's suitable for the software based renderer we're going to use.

## Application Structure

Typically, a graphical application in hosted environments has at least three different tasks:

 * Receives user input from operation system APIs.
 * Reacts to the input by performing application specific computations.
 * Renders an updated user interface and presents it on the screen using device-independent operating system APIs.

The operating system provides an event loop to connect and schedule these tasks. Slint implements the
task of receiving user input and forwarding it to the user interface layer, and rendering the user interface to the screen.

In bare metal environments it's your responsibility to substitute and connect functionality that's otherwise provided by the operating system:

 * Select crates that allow you to initialize the chips that operate peripherals, such as a touch input or display controller.
   If there are no crates, you may have to to develop your own drivers.
 * Drive the event loop yourself by querying peripherals for input, forwarding that input into computational modules of your
   application and instructing Slint to render the user interface.

In Slint, the two primary APIs you need to use to accomplish these tasks are the [`slint::platform::Platform`] trait and the [`slint::Window`] struct.
In the following sections we're going to cover how to use them and how they integrate into your event loop.

### The `Platform` Trait

The [`slint::platform::Platform`] trait defines the interface between Slint and platform APIs typically provided by operating and windowing systems.

You need to provide a minimal implementation of this trait and call [`slint::platform::set_platform`] before constructing your Slint application.

This minimal implementation needs to cover two functions:

 * `fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter + 'static>, PlatformError>;`: Implement this function to return an implementation of the `WindowAdapter`
   trait that will be associated with the Slint components you create. We provide a convenience struct [`slint::platform::software_renderer::MinimalSoftwareWindow`]
   that implements this trait.
 * `fn duration_since_start(&self) -> Duration`: For animations in `.slint` design files to change properties correctly, Slint needs to know
   how much time has elapsed between two rendered frames. In a bare metal environment you need to provide a source of time. Often the HAL crate of your
   device provides a system timer API for this, which you can query in your implementation.

You may override more functions of this trait, for example to handle debug output, to delegate the event loop,
or to deliver events in multi-threaded environments.

A typical minimal implementation of the [`Platform`] trait that uses the [`MinimalSoftwareWindow`] looks like this:

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
    window: Rc<MinimalSoftwareWindow>,
    // optional: some timer device from your device's HAL crate
    timer: hal::Timer,
    // ... maybe more devices
}

impl Platform for MyPlatform {
    fn create_window_adapter(&self) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        // Since on MCUs, there can be only one window, just return a clone of self.window.
        // We'll also use the same window in the event loop.
        Ok(self.window.clone())
    }
    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_micros(self.timer.get_time())
    }
    // optional: You can put the event loop there, or in the main function, see later
    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        todo!();
    }
}

// #[hal::entry]
fn main() {
    // Initialize the heap allocator, peripheral devices and other things.
    // ...

    // Initialize a window (we'll need it later).
    let window = MinimalSoftwareWindow::new(Default::default());
    slint::platform::set_platform(Box::new(MyPlatform {
        window: window.clone(),
        timer: hal::Timer(/*...*/),
        //...
    }))
    .unwrap();

    // Setup the UI.
    let ui = MyUI::new();
    // ... setup callback and properties on `ui` ...

    // Make sure the window covers our entire screen.
    window.set_size(slint::PhysicalSize::new(320, 240));

    // ... start event loop (see later) ...
}
```

### The Event Loop

With a `Platform` in place, you can write the main event loop to drive all the different tasks.

You can choose between two options:

 * You can implement [`slint::platform::Platform::run_event_loop`]: Use this if you want to start the
   event loop in a way similar to desktop platforms, using the [`run()`](slint::ComponentHandle::run) function
   of your component, or use [`slint::run_event_loop()`]. Both of these functions will call your implementation
   of [`slint::platform::Platform::run_event_loop`].
 * Implement a `loop { ... }` directly in your main function: This is called a super loop architecture and common
   for programs running in bare metal environments on MCUs. It allows you to initialize you device peripherals
   and access them without the need to move them into your `Platform` implementation.

A typical super loop with Slint combines the tasks of querying input drivers, application specific computations,
rendering and possibly putting the device into a low-power sleep state. Below is an example:

```rust,no_run
use slint::platform::software_renderer::MinimalSoftwareWindow;
let window = MinimalSoftwareWindow::new(Default::default());
# fn check_for_touch_event() -> Option<slint::platform::WindowEvent> { todo!() }
# mod hal { pub fn wfi() {} }
//...
loop {
    // Let Slint run the timer hooks and update animations.
    slint::platform::update_timers_and_animations();

    // Check the touch screen or input device using your driver.
    if let Some(event) = check_for_touch_event(/*...*/) {
        // convert the event from the driver into a `slint::platform::WindowEvent`
        // and pass it to the window.
        window.try_dispatch_event(event).unwrap();
    }

    // ... maybe some more application logic ...

    // Draw the scene if something needs to be drawn.
    window.draw_if_needed(|renderer| {
        // see next section about rendering.
        todo!()
    });

    // Try to put the MCU to sleep
    if !window.has_active_animations() {
        if let Some(duration) = slint::platform::duration_until_next_timer_update() {
            // ... schedule a timer interrupt in `duration` ...
        }
        hal::wfi(); // Wait for interrupt
    }
}

```

### The Renderer

In desktop and embedded environments, Slint typically uses operating system provided APIs to render the user interface using the GPU.
In contrast, most MCUs don't have GPUs. Instead, software rendering is used where all rendering is done by software on the CPU.
Slint provides a SoftwareRenderer for this task.

In the earlier example, we've instantiated a [`slint::platform::software_renderer::MinimalSoftwareWindow`]. This struct implements the
`slint::platform::WindowAdapter` trait and also holds an instance of a [`slint::platform::software_renderer::SoftwareRenderer`]. You access it
through the callback parameter of the [`draw_if_needed()`](MinimalSoftwareWindow::draw_if_needed) function.
Depending on the amount of RAM your MCU has, and the kind of screen attached, you can choose between two different ways of using the renderer:

 * Use the [`SoftwareRenderer::render()`] function if you have enough RAM to allocate one, or even two, copies of the entire screen (also known as
   frame buffer).
 * Use the [`SoftwareRenderer::render_by_line()`] function to render the entire user interface line by line and send each line of pixels to the screen,
   typically via the SPI. This requires allocating at least enough RAM to store one single line of pixels.

With both methods Slint renders into a provided buffer, which is a slice of a type that implements the [`slint::platform::software_renderer::TargetPixel`] trait.
For convenience, Slint provides an implementation for [`slint::Rgb8Pixel`] and [`slint::platform::software_renderer::Rgb565Pixel`].

#### Rendering Into a Buffer

The following example uses double buffering and swaps between two buffers. This
requires a graphics driver that takes the address of the currently displayed
frame buffer, also known as front buffer. A dedicated chip is then responsible
for reading from RAM and transferring the contents to the attached screen,
without any interference of the CPU. Meanwhile, Slint renders into the second
buffer, the back buffer.

```rust,no_run
use slint::platform::software_renderer::Rgb565Pixel;
# fn is_swap_pending()->bool {false} fn swap_buffers() {}

// In this example, we have two buffer: one is currently displayed, and we are
// rendering into the second one. Hence we use `RepaintBufferType::SwappedBuffers`
let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
    slint::platform::software_renderer::RepaintBufferType::SwappedBuffers
);

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

#### Rendering Line by Line

When rendering the user interface line by line, you need to implement the [`LineBufferProvider`] trait. It
defines a bi-directional interface between Slint and your code to send lines to the screen:

* The trait's associated `TargetPixel` type let's Slint know how to create and manipulate pixels. How exactly the pixels are
  represented in your device and how they are blended remains your implementation detail.
* The trait's `process_line` function notifies you when a line can be rendered and provides a callback that you can invoke
  to fill a slice of pixels for the given line.

The following example defines a `DisplayWrapper` struct: It connects screen driver that implements the [`embedded_graphics`](https://lib.rs/embedded-graphics) traits
with Slint's `Rgb565Pixel` type to implement the `LineBufferProvider` trait. The pixels for one line are sent to the screen by calling
the [DrawTarget::fill_contiguous](https://docs.rs/embedded-graphics/0.7.1/embedded_graphics/draw_target/trait.DrawTarget.html) function.

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
}
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

// Note that we use `ReusedBuffer` as parameter for MinimalSoftwareWindow to indicate
// that we just need to re-render what changed since the last frame.
// What's shown on the screen buffer is not in our RAM, but actually within the display itself.
// Only the changed part of the screen will be updated.
let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
    slint::platform::software_renderer::RepaintBufferType::ReusedBuffer
);

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

Note: In our experience, using the synchronous `DrawTarget::fill_contiguous` function is slow. If
your device is capable of using DMA, you may be able to achieve better performance by using
two line buffers: One buffer to render into with the CPU, while the other buffer is transferred to
the screen using DMA asynchronously.

## Example Implementations

The examples that come with Slint use a helper crate called `mcu-board-support`. It provides implementations of
the `Platform` trait for some MCUs, along with support for touch input and system timers.

You can find the crate in our Git repository at:

<https://github.com/slint-ui/slint/tree/master/examples/mcu-board-support>

If your MCU is among the supported boards, then you can use it by specifying it as a
[dependency from our Git repository](https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories)
in your `Cargo.toml`.

For an entire template, check out our [Slint Bare Metal Microcontroller Rust Template](https://github.com/slint-ui/slint-mcu-rust-template).

We also have a version of our printer demo that we've adapted to small screens, the [MCU Printer Demo](https://github.com/slint-ui/slint/tree/master/demos/printerdemo_mcu).
