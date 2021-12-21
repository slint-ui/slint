// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: (GPL-3.0-only OR LicenseRef-SixtyFPS-commercial)

/*!

**NOTE**: This library is an **internal** crate for the [SixtyFPS project](https://sixtyfps.io).
This crate should **not be used directly** by applications using SixtyFPS.
You should use the `sixtyfps` crate instead.

**WARNING**: This crate does not follow the semver convention for versioning and can
only be used with `version = "=x.y.z"` in Cargo.toml.

*/
#![doc(html_logo_url = "https://sixtyfps.io/resources/logo.drawio.svg")]
#![recursion_limit = "1024"]

extern crate alloc;

use sixtyfps_corelib::graphics::{Image, Size};
#[cfg(not(no_qt))]
use sixtyfps_corelib::items::ImageFit;
use sixtyfps_corelib::window::Window;
#[cfg(not(no_qt))]
use sixtyfps_corelib::ImageInner;

#[cfg(not(no_qt))]
mod qt_widgets;
#[cfg(not(no_qt))]
mod qt_window;

mod key_generated;

#[doc(hidden)]
#[cold]
#[cfg(not(target_arch = "wasm32"))]
pub fn use_modules() -> usize {
    #[cfg(no_qt)]
    {
        ffi::sixtyfps_qt_get_widget as usize
    }
    #[cfg(not(no_qt))]
    {
        qt_window::ffi::sixtyfps_qt_get_widget as usize
            + (&qt_widgets::NativeButtonVTable) as *const _ as usize
    }
}

#[cfg(no_qt)]
mod ffi {
    #[no_mangle]
    pub extern "C" fn sixtyfps_qt_get_widget(
        _: &sixtyfps_corelib::window::WindowRc,
    ) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}

/// NativeWidgets and NativeGlobals are "type list" containing all the native widgets and global types.
///
/// It is built as a tuple `(Type, Tail)`  where `Tail` is also a "type list". a `()` is the end.
///
/// So it can be used like this to do something for all types:
///
/// ```rust
/// trait DoSomething {
///     fn do_something(/*...*/) { /*...*/
///     }
/// }
/// impl DoSomething for () {}
/// impl<T: sixtyfps_corelib::rtti::BuiltinItem, Next: DoSomething> DoSomething for (T, Next) {
///     fn do_something(/*...*/) {
///          /*...*/
///          Next::do_something(/*...*/);
///     }
/// }
/// sixtyfps_rendering_backend_qt::NativeWidgets::do_something(/*...*/)
/// ```
#[cfg(not(no_qt))]
#[rustfmt::skip]
pub type NativeWidgets =
    (qt_widgets::NativeButton,
    (qt_widgets::NativeCheckBox,
    (qt_widgets::NativeSlider,
    (qt_widgets::NativeSpinBox,
    (qt_widgets::NativeGroupBox,
    (qt_widgets::NativeLineEdit,
    (qt_widgets::NativeScrollView,
    (qt_widgets::NativeStandardListViewItem,
    (qt_widgets::NativeComboBox,
    (qt_widgets::NativeComboBoxPopup,
    (qt_widgets::NativeTabWidget,
    (qt_widgets::NativeTab,
            ()))))))))))));

#[cfg(not(no_qt))]
#[rustfmt::skip]
pub type NativeGlobals =
    (qt_widgets::NativeStyleMetrics,
        ());

pub mod native_widgets {
    #[cfg(not(no_qt))]
    pub use super::qt_widgets::*;
}

#[cfg(no_qt)]
pub type NativeWidgets = ();
#[cfg(no_qt)]
pub type NativeGlobals = ();

pub const HAS_NATIVE_STYLE: bool = cfg!(not(no_qt));
/// False if the backend was compiled without Qt so it wouldn't do anything
pub const IS_AVAILABLE: bool = cfg!(not(no_qt));

pub struct Backend;
impl sixtyfps_corelib::backend::Backend for Backend {
    fn create_window(&'static self) -> std::rc::Rc<Window> {
        #[cfg(no_qt)]
        panic!("The Qt backend needs Qt");
        #[cfg(not(no_qt))]
        {
            sixtyfps_corelib::window::Window::new(|window| qt_window::QtWindow::new(window))
        }
    }

    fn run_event_loop(&'static self, _behavior: sixtyfps_corelib::backend::EventLoopQuitBehavior) {
        #[cfg(not(no_qt))]
        {
            let quit_on_last_window_closed = match _behavior {
                sixtyfps_corelib::backend::EventLoopQuitBehavior::QuitOnLastWindowClosed => true,
                sixtyfps_corelib::backend::EventLoopQuitBehavior::QuitOnlyExplicitly => false,
            };
            // Schedule any timers with Qt that were set up before this event loop start.
            crate::qt_window::timer_event();
            use cpp::cpp;
            cpp! {unsafe [quit_on_last_window_closed as "bool"] {
                ensure_initialized(true);
                qApp->setQuitOnLastWindowClosed(quit_on_last_window_closed);
                qApp->exec();
            } }
        };
    }

    fn quit_event_loop(&'static self) {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            cpp! {unsafe [] {
                // Use a quit event to avoid qApp->quit() calling
                // [NSApp terminate:nil] and us never returning from the
                // event loop - sixtyfps-viewer relies on the ability to
                // return from run().
                QCoreApplication::postEvent(qApp, new QEvent(QEvent::Quit));
            } }
        };
    }

    fn register_font_from_memory(
        &'static self,
        _data: &'static [u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            let data = qttypes::QByteArray::from(_data);
            cpp! {unsafe [data as "QByteArray"] {
                ensure_initialized(true);
                QFontDatabase::addApplicationFontFromData(data);
            } }
        };
        Ok(())
    }

    fn register_font_from_path(
        &'static self,
        _path: &std::path::Path,
    ) -> Result<(), Box<dyn std::error::Error>> {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            let encoded_path: qttypes::QByteArray = _path.to_string_lossy().as_bytes().into();
            cpp! {unsafe [encoded_path as "QByteArray"] {
                ensure_initialized(true);

                QString requested_path = QFileInfo(QFile::decodeName(encoded_path)).canonicalFilePath();
                static QSet<QString> loaded_app_fonts;
                // QFontDatabase::addApplicationFont unconditionally reads the provided file from disk,
                // while we want to do this only once to avoid things like the live-review going crazy.
                if (!loaded_app_fonts.contains(requested_path)) {
                    loaded_app_fonts.insert(requested_path);
                    QFontDatabase::addApplicationFont(requested_path);
                }
            } }
        };
        Ok(())
    }

    fn set_clipboard_text(&'static self, _text: String) {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            let text: qttypes::QString = _text.into();
            cpp! {unsafe [text as "QString"] {
                ensure_initialized();
                QGuiApplication::clipboard()->setText(text);
            } }
        }
    }

    fn clipboard_text(&'static self) -> Option<String> {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            let has_text = cpp! {unsafe [] -> bool as "bool" {
                ensure_initialized();
                return QGuiApplication::clipboard()->mimeData()->hasText();
            } };
            if has_text {
                return Some(
                    cpp! { unsafe [] -> qttypes::QString as "QString" {
                        return QGuiApplication::clipboard()->text();
                    }}
                    .into(),
                );
            }
        }
        None
    }

    fn post_event(&'static self, _event: Box<dyn FnOnce() + Send>) {
        #[cfg(not(no_qt))]
        {
            use cpp::cpp;
            cpp! {{
               struct TraitObject { void *a, *b; };
               struct EventHolder {
                   TraitObject fnbox = {nullptr, nullptr};
                   ~EventHolder() {
                       if (fnbox.a != nullptr || fnbox.b != nullptr) {
                           rust!(SFPS_delete_event_holder [fnbox: *mut dyn FnOnce() as "TraitObject"] {
                               drop(Box::from_raw(fnbox))
                           });
                       }
                   }
                   EventHolder(TraitObject f) : fnbox(f)  {}
                   EventHolder(const EventHolder&) = delete;
                   EventHolder& operator=(const EventHolder&) = delete;
                   EventHolder(EventHolder&& other) : fnbox(other.fnbox) {
                        other.fnbox = {nullptr, nullptr};
                   }
                   void operator()() {
                        if (fnbox.a != nullptr || fnbox.b != nullptr) {
                            TraitObject fnbox = std::move(this->fnbox);
                            this->fnbox = {nullptr, nullptr};
                            rust!(SFPS_call_event_holder [fnbox: *mut dyn FnOnce() as "TraitObject"] {
                               let b = Box::from_raw(fnbox);
                               b();
                            });
                        }
                   }
               };
            }};
            let fnbox = Box::into_raw(_event);
            cpp! {unsafe [fnbox as "TraitObject"] {
                QTimer::singleShot(0, qApp, EventHolder{fnbox});
            }}
        };
    }

    fn image_size(&'static self, _image: &Image) -> Size {
        #[cfg(not(no_qt))]
        {
            let inner: &ImageInner = _image.into();
            match inner {
                sixtyfps_corelib::ImageInner::None => Default::default(),
                sixtyfps_corelib::ImageInner::EmbeddedImage(buffer) => {
                    [buffer.width() as _, buffer.height() as _].into()
                }
                _ => qt_window::load_image_from_resource(inner, None, ImageFit::fill)
                    .map(|img| {
                        let qsize = img.size();
                        euclid::size2(qsize.width as f32, qsize.height as f32)
                    })
                    .unwrap_or_default(),
            }
        }
        #[cfg(no_qt)]
        Default::default()
    }
}
