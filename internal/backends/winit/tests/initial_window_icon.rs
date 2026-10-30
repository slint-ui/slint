// Copyright © 2026 Koichiro Ohba
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// cSpell: ignore Koichiro Ohba

#[cfg(target_os = "linux")]
fn main() {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use slint::ComponentHandle;
    use slint::winit_030::WinitWindowAccessor;
    use x11rb::protocol::xproto::{AtomEnum, ConnectionExt, MapState};

    slint::slint! {
        #[edition = "1.19"]
        export component App inherits Window {
            width: 100px;
            height: 100px;
            in property <image> window-icon;
            icon: root.window-icon;
        }
    }

    // Recreate the native window on X11, as the backend also does when resuming.
    unsafe { std::env::set_var("SLINT_DESTROY_WINDOW_ON_HIDE", "1") };
    let mut event_loop = slint::winit_030::winit::event_loop::EventLoop::<
        i_slint_backend_winit::SlintEvent,
    >::with_user_event();
    slint::winit_030::winit::platform::x11::EventLoopBuilderExtX11::with_x11(&mut event_loop);
    let backend = i_slint_backend_winit::Backend::builder()
        .with_event_loop_builder(event_loop)
        .with_renderer_name("software")
        .build()
        .unwrap();
    slint::platform::set_platform(Box::new(backend)).unwrap();

    slint::spawn_local(async move {
        let (connection, _) = x11rb::connect(None).unwrap();
        let icon_atom =
            connection.intern_atom(false, b"_NET_WM_ICON").unwrap().reply().unwrap().atom;
        let app = App::new().unwrap();
        let mut pixels = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(2, 2);
        for pixel in pixels.make_mut_slice() {
            *pixel = slint::Rgba8Pixel { r: 0x12, g: 0x34, b: 0x56, a: 0xff };
        }
        app.set_window_icon(slint::Image::from_rgba8(pixels));
        let check_icon = |native: &slint::winit_030::winit::window::Window| {
            let RawWindowHandle::Xlib(handle) = native.window_handle().unwrap().as_raw() else {
                panic!("expected an X11 window");
            };
            let window = u32::try_from(handle.window).unwrap();
            let property = connection
                .get_property(false, window, icon_atom, AtomEnum::CARDINAL, 0, 6)
                .unwrap()
                .reply()
                .unwrap();
            let icon: Vec<_> = property.value32().into_iter().flatten().collect();
            assert_eq!(
                icon,
                [2, 2, 0xff123456, 0xff123456, 0xff123456, 0xff123456],
                "the native window has no correct icon"
            );
            window
        };
        let native = app.window().winit_window().await.unwrap();
        let window = check_icon(&native);
        assert_eq!(
            connection.get_window_attributes(window).unwrap().reply().unwrap().map_state,
            MapState::UNMAPPED
        );
        let initial_id = native.id();
        drop(native);
        app.show().unwrap();
        app.hide().unwrap();
        app.show().unwrap();
        let native = app.window().winit_window().await.unwrap();
        assert_ne!(native.id(), initial_id, "the native window was not recreated");
        check_icon(&native);
        drop(native);
        app.hide().unwrap();
        slint::quit_event_loop().unwrap();
    })
    .unwrap();
    slint::run_event_loop_until_quit().unwrap();
}

#[cfg(not(target_os = "linux"))]
fn main() {}
