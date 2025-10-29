use std::{cell::RefCell, collections::HashMap, ptr::NonNull, rc::Weak};

use block2::RcBlock;
use objc2_foundation::{NSNotificationCenter, NSOperationQueue, NSNotification, NSNumber, NSValue};
use objc2_ui_kit::{UICoordinateSpace, UIScreen, UIViewAnimationCurve};
use raw_window_handle::{HasWindowHandle};
use i_slint_core::{animations::EasingCurve, api::{LogicalPosition, LogicalSize}};
use winit::window::WindowId;

use crate::winitwindowadapter::WinitWindowAdapter;

#[derive(Debug, Clone)]
struct FrameTransition {
    pub begin_origin: LogicalPosition,
    pub begin_size: LogicalSize,
    pub end_origin: LogicalPosition,
    pub end_size: LogicalSize,
}

pub(crate) struct KeyboardNotifications([objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2_foundation::NSObjectProtocol>>; 3]);

impl Drop for KeyboardNotifications {
    fn drop(&mut self) {
        for notification_object in &self.0 {
            unsafe {
                objc2_foundation::NSNotificationCenter::defaultCenter().removeObserver(notification_object.as_ref());
            }
        }
    }
}

pub(crate) fn register_keyboard_notifications(active_windows: Weak<RefCell<HashMap<WindowId, Weak<WinitWindowAdapter>>>>) -> KeyboardNotifications {
    let event_block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        if let Some(active_windows) = active_windows.upgrade() {
            crate::virtual_keyboard::handle_keyboard_notification(
                unsafe { notification.as_ref() },
                active_windows.borrow().values()
            );
        }
    });
    let default_center = NSNotificationCenter::defaultCenter();
    let main_queue = NSOperationQueue::mainQueue();
    KeyboardNotifications(unsafe {
        [
            objc2_ui_kit::UIKeyboardWillShowNotification,
            objc2_ui_kit::UIKeyboardWillHideNotification,
            objc2_ui_kit::UIKeyboardWillChangeFrameNotification
        ].map(|name|
            default_center.addObserverForName_object_queue_usingBlock(
                Some(name), None, Some(&main_queue), &event_block,
            )
        )
    })
}

pub(crate) fn handle_keyboard_notification<'a>(notification: &NSNotification, windows: impl IntoIterator<Item = &'a Weak<WinitWindowAdapter>>) -> Option<()> {
    let screen = notification.object()?.downcast::<UIScreen>().ok()?;
    let coordinate_space = screen.coordinateSpace();
    let user_info = notification.userInfo()?;

    let frame_begin = unsafe { user_info.objectForKey(objc2_ui_kit::UIKeyboardFrameBeginUserInfoKey)?.downcast::<NSValue>().ok()?.rectValue() };
    let frame_end = unsafe { user_info.objectForKey(objc2_ui_kit::UIKeyboardFrameEndUserInfoKey)?.downcast::<NSValue>().ok()?.rectValue() };

    let animation_curve = match UIViewAnimationCurve(user_info.objectForKey(unsafe { objc2_ui_kit::UIKeyboardAnimationCurveUserInfoKey })?.downcast::<NSNumber>().ok()?.as_isize()) {
        UIViewAnimationCurve::EaseInOut => EasingCurve::CubicBezier([0.42, 0.0, 0.58, 1.0]),
        UIViewAnimationCurve::EaseIn => EasingCurve::CubicBezier([0.42, 0.0, 1.0, 1.0]),
        UIViewAnimationCurve::EaseOut => EasingCurve::CubicBezier([0.0, 0.0, 0.58, 1.0]),
        _ => return None,
    };

    let animation_duration = user_info.objectForKey(unsafe { objc2_ui_kit::UIKeyboardAnimationDurationUserInfoKey })?.downcast::<NSNumber>().ok()?.as_f64();
    let is_local = user_info.objectForKey(unsafe { objc2_ui_kit::UIKeyboardIsLocalUserInfoKey })?.downcast::<NSNumber>().ok()?.as_bool();

    let adapter_frames = windows.into_iter().filter_map(|adapter| {
            let adapter = adapter.upgrade()?;
            let raw_window_handle::RawWindowHandle::UiKit(window_handle) = adapter.winit_window()?.window_handle().ok()?.as_raw() else {
                return None;
            };
            let view = unsafe { &*(window_handle.ui_view.as_ptr() as *const objc2_ui_kit::UIView) };
            let frame_begin = view.convertRect_fromCoordinateSpace(frame_begin, &coordinate_space);
            let begin_origin = LogicalPosition::new(frame_begin.origin.x as _, frame_begin.origin.y as _);
            let begin_size = LogicalSize::new(frame_begin.size.width as _, frame_begin.size.height as _);

            let frame_end = view.convertRect_fromCoordinateSpace(frame_end, &coordinate_space);
            let end_origin = LogicalPosition::new(frame_end.origin.x as _, frame_end.origin.y as _);
            let end_size = LogicalSize::new(frame_end.size.width as _, frame_end.size.height as _);

            Some((adapter, FrameTransition { begin_origin, begin_size, end_origin, end_size }))
        });

    let name = notification.name();
    if name.isEqualToString(unsafe { objc2_ui_kit::UIKeyboardWillShowNotification }) {
        for (adapter, frame_transition) in adapter_frames {
            adapter.ios_keyboard_will_show(animation_curve, animation_duration, frame_transition, is_local);
        }
    } else if name.isEqualToString(unsafe { objc2_ui_kit::UIKeyboardWillHideNotification }) {
        for (adapter, frame_transition) in adapter_frames {
            adapter.ios_keyboard_will_hide(animation_curve, animation_duration, frame_transition, is_local);
        }
    } else if name.isEqualToString(unsafe { objc2_ui_kit::UIKeyboardWillChangeFrameNotification }) {
        for (adapter, frame_transition) in adapter_frames {
            adapter.ios_keyboard_will_change_frame(animation_curve, animation_duration, frame_transition, is_local);
        }
    }

    Some(())
}

impl crate::winitwindowadapter::WinitWindowAdapter {
    #[cfg(target_os = "ios")]
    fn ios_keyboard_will_show(&self,
        animation_curve: i_slint_core::animations::EasingCurve,
        animation_duration: f64,
        frame_transition: crate::virtual_keyboard::FrameTransition,
        is_local: bool,
    ) {
        if !is_local {
            return;
        }
        todo!()
    }
    #[cfg(target_os = "ios")]
    fn ios_keyboard_will_hide(&self,
        animation_curve: i_slint_core::animations::EasingCurve,
        animation_duration: f64,
        frame_transition: crate::virtual_keyboard::FrameTransition,
        is_local: bool,
    ) {
        if !is_local {
            return;
        }
        todo!()
    }
    #[cfg(target_os = "ios")]
    fn ios_keyboard_will_change_frame(&self,
        animation_curve: i_slint_core::animations::EasingCurve,
        animation_duration: f64,
        frame_transition: crate::virtual_keyboard::FrameTransition,
        is_local: bool,
    ) {
        if !is_local {
            return;
        }
        todo!()
    }
}