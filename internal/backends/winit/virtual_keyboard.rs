// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use std::{cell::RefCell, collections::HashMap, ptr::NonNull, rc::Weak};

use block2::RcBlock;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSNumber, NSOperationQueue, NSValue};
use objc2_ui_kit::{UICoordinateSpace, UIScreen, UIViewAnimationCurve};
use raw_window_handle::HasWindowHandle;
use winit::window::WindowId;

use crate::winitwindowadapter::WinitWindowAdapter;

mod keyboard_animator;
pub(crate) use keyboard_animator::KeyboardCurveSampler;

pub(crate) struct KeyboardNotifications(
    [objc2::rc::Retained<objc2::runtime::ProtocolObject<dyn objc2_foundation::NSObjectProtocol>>;
        3],
);

impl Drop for KeyboardNotifications {
    fn drop(&mut self) {
        for notification_object in &self.0 {
            unsafe {
                objc2_foundation::NSNotificationCenter::defaultCenter()
                    .removeObserver(notification_object.as_ref());
            }
        }
    }
}

pub(crate) fn register_keyboard_notifications(
    active_windows: Weak<RefCell<HashMap<WindowId, Weak<WinitWindowAdapter>>>>,
) -> KeyboardNotifications {
    let event_block = RcBlock::new(move |notification: NonNull<NSNotification>| {
        if let Some(active_windows) = active_windows.upgrade() {
            crate::virtual_keyboard::handle_keyboard_notification(
                unsafe { notification.as_ref() },
                active_windows.borrow().values(),
            );
        }
    });
    let default_center = NSNotificationCenter::defaultCenter();
    let main_queue = NSOperationQueue::mainQueue();
    KeyboardNotifications(unsafe {
        [
            objc2_ui_kit::UIKeyboardWillShowNotification,
            objc2_ui_kit::UIKeyboardWillHideNotification,
            objc2_ui_kit::UIKeyboardWillChangeFrameNotification,
        ]
        .map(|name| {
            default_center.addObserverForName_object_queue_usingBlock(
                Some(name),
                None,
                Some(&main_queue),
                &event_block,
            )
        })
    })
}

pub(crate) fn handle_keyboard_notification<'a>(
    notification: &NSNotification,
    windows: impl IntoIterator<Item = &'a Weak<WinitWindowAdapter>>,
) -> Option<()> {
    let user_info = notification.userInfo()?;
    let is_local = user_info
        .objectForKey(unsafe { objc2_ui_kit::UIKeyboardIsLocalUserInfoKey })?
        .downcast::<NSNumber>()
        .ok()?
        .as_bool();
    if !is_local {
        return Some(());
    }
    let screen = notification.object()?.downcast::<UIScreen>().ok()?;
    let coordinate_space = screen.coordinateSpace();

    let frame_begin = unsafe {
        user_info
            .objectForKey(objc2_ui_kit::UIKeyboardFrameBeginUserInfoKey)?
            .downcast::<NSValue>()
            .ok()?
            .rectValue()
    };
    let frame_end = unsafe {
        user_info
            .objectForKey(objc2_ui_kit::UIKeyboardFrameEndUserInfoKey)?
            .downcast::<NSValue>()
            .ok()?
            .rectValue()
    };
    let animation_duration = user_info
        .objectForKey(unsafe { objc2_ui_kit::UIKeyboardAnimationDurationUserInfoKey })?
        .downcast::<NSNumber>()
        .ok()?
        .as_f64();
    let curve = UIViewAnimationCurve(
        user_info
            .objectForKey(unsafe { objc2_ui_kit::UIKeyboardAnimationCurveUserInfoKey })?
            .downcast::<NSNumber>()
            .unwrap()
            .as_isize(),
    );

    let name = notification.name();
    if name.isEqualToString(unsafe { objc2_ui_kit::UIKeyboardWillChangeFrameNotification }) {
        for adapter in windows.into_iter() {
            let adapter = adapter.upgrade()?;
            let raw_window_handle::RawWindowHandle::UiKit(window_handle) =
                adapter.winit_window()?.window_handle().ok()?.as_raw()
            else {
                return None;
            };
            let view = unsafe { &*(window_handle.ui_view.as_ptr() as *const objc2_ui_kit::UIView) };
            let frame_begin = view.convertRect_fromCoordinateSpace(frame_begin, &coordinate_space);
            let frame_end = view.convertRect_fromCoordinateSpace(frame_end, &coordinate_space);

            adapter.with_keyboard_curve_sampler(|kcs| {
                kcs.start(animation_duration, curve, frame_begin, frame_end);
            });
        }
    }

    Some(())
}
