//! Functions usefull for testing

/// SixtyFPS animations do not use real time, but use a mocked time.
/// Normally, the event loop update the time of the animation using
/// real time, but in tests, it is more convinient to use the fake time.
/// This function will add some milliseconds to the fake time
#[no_mangle]
pub extern "C" fn sixtyfps_test_ellapse_time(time_in_ms: u64) {
    crate::animations::CURRENT_ANIMATION_DRIVER.with(|driver| {
        let mut tick = driver.current_tick();
        tick += instant::Duration::from_millis(time_in_ms);
        driver.update_animations(tick)
    })
}

/// Somehow this is required for the extern "C" things to be exported in a dependent dynlib
#[doc(hidden)]
pub fn dummy() {
    #[derive(Clone)]
    struct Foo;
    foo(Foo);
    fn foo(f: impl Clone) {
        let _ = f.clone();
    }
}
