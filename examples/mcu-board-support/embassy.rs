// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::{Cell, RefCell};

pub(crate) trait PlatformBackend {
    async fn dispatch_events(&mut self, window: &slint::Window);
    async fn render(&mut self, renderer: &slint::platform::software_renderer::SoftwareRenderer);
}

pub struct EmbassyBackend<PlatformImpl> {
    window: RefCell<Option<Rc<slint::platform::software_renderer::MinimalSoftwareWindow>>>,
    window_changed: Cell<bool>,
    display_size: slint::PhysicalSize,
    platform_backend: RefCell<PlatformImpl>,
}

impl<PlatformImpl> EmbassyBackend<PlatformImpl> {
    pub fn new(platform_backend: PlatformImpl, display_size: slint::PhysicalSize) -> Self {
        Self {
            window: RefCell::default(),
            window_changed: Default::default(),
            display_size,
            platform_backend: RefCell::new(platform_backend),
        }
    }
}

impl<PlatformImpl: PlatformBackend + 'static> slint::platform::Platform
    for EmbassyBackend<PlatformImpl>
{
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::SwappedBuffers,
        );
        window.set_size(self.display_size.to_logical(window.scale_factor()));
        self.window.replace(Some(window.clone()));
        self.window_changed.set(true);
        Ok(window)
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let mut executor = embassy_executor::Executor::new();
        let static_executor: &'static mut embassy_executor::Executor =
            unsafe { core::mem::transmute(&mut executor) };

        static_executor.run(|spawner| {
            let this = unsafe {
                core::mem::transmute::<
                    &'_ EmbassyBackend<PlatformImpl>,
                    &'static EmbassyBackend<PlatformImpl>,
                >(self)
            };
            spawner.must_spawn(main_loop_task(Box::pin(this.run_loop())));
        });
    }

    fn new_event_loop_proxy(&self) -> Option<Box<dyn slint::platform::EventLoopProxy>> {
        None
    }

    fn duration_since_start(&self) -> core::time::Duration {
        embassy_time::Instant::now().duration_since(embassy_time::Instant::from_secs(0)).into()
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        defmt::println!("{}", defmt::Display2Format(&arguments));
    }
}

impl<PlatformImpl: PlatformBackend> EmbassyBackend<PlatformImpl> {
    async fn run_loop(&self) {
        let mut platform_backend = self.platform_backend.borrow_mut();

        loop {
            slint::platform::update_timers_and_animations();

            if self.window_changed.take() {
                let window = self.window.borrow();
                let window = window.as_ref().unwrap();
                window.set_size(self.display_size.to_logical(window.scale_factor()));
            }

            let maybe_window = (*self.window.borrow()).clone();

            if let Some(window) = maybe_window {
                window
                    .draw_async_if_needed(async |renderer| {
                        platform_backend.render(renderer).await;
                    })
                    .await;

                // TODO: poll this at the same time as the timer for the next animation, when we can poll
                // the gt911 driver without errors in parallel (first time works, second poll causes errors).
                platform_backend.dispatch_events(&window).await;
            }

            /*
            Use this when the async gt911 API works:

            if let Some(duration) = slint::platform::duration_until_next_timer_update()
                .and_then(|core_duration| embassy_time::Duration::try_from(core_duration).ok())
            {
                embassy_time::Timer::after(duration).await;
            }
            */
        }
    }
}

#[embassy_executor::task()]
async fn main_loop_task(
    run_fn: core::pin::Pin<Box<dyn core::future::Future<Output = ()> + 'static>>,
) {
    run_fn.await;
}
