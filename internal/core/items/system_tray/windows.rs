// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Windows system tray backend using the `Shell_NotifyIconW` API directly.
//!
//! Everything here runs on the Slint event-loop thread, which on Windows is the
//! same thread that owns the winit message pump. The hidden message-only window
//! we create for tray callbacks therefore delivers its messages through the
//! usual `PeekMessage` loop that winit already spins.

use super::{Error, Params};
use crate::SharedVector;
use crate::graphics::Image;
use crate::item_tree::ItemWeak;
use crate::items::MenuEntry;
use crate::menus::MenuVTable;

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateBitmap, CreateDIBSection, DIB_RGB_COLORS,
    DeleteObject, GetDC, ReleaseDC,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, GWLP_USERDATA, GetCursorPos, GetWindowLongPtrW, HICON, HMENU,
    HWND_MESSAGE, ICONINFO, MF_DISABLED, MF_GRAYED, MF_POPUP, MF_SEPARATOR, MF_STRING,
    PostMessageW, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow, SetWindowLongPtrW,
    TPM_BOTTOMALIGN, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_APP, WM_CONTEXTMENU, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP,
    WNDCLASSW,
};
use windows::core::{HSTRING, PCWSTR, w};

// Mirror the other backends' depth cap so a pathological menu tree can't blow the stack.
const MAX_DEPTH: usize = 15;
// First menu command id; entry index is added to this. Command ids must be < 0xF000
// (the range Windows reserves for system commands) and non-zero.
const MENU_CMD_BASE: u32 = 0x100;
const WM_TRAYICON: u32 = WM_APP + 1;
const TRAY_UID: u32 = 1;

// ---------------------------------------------------------------------------
// Inner: the per-tray state reachable from both `PlatformTray` and the wnd
// proc. Heap-allocated via `Box` so the address we stash in `GWLP_USERDATA`
// stays valid while the `PlatformTray` is alive.
// ---------------------------------------------------------------------------

struct Inner {
    hwnd: HWND,
    hicon: HICON,
    self_weak: ItemWeak,
    hmenu: Cell<Option<HMENU>>,
    // A UTF-16 copy of the tooltip so NIM_ADD can be re-issued on TaskbarCreated
    // without touching the Slint property tree from the wnd proc. Written once in
    // the constructor and otherwise read-only.
    tip: HSTRING,
}

impl Inner {
    fn activate(&self, entry_index: usize) {
        let Some(item_rc) = self.self_weak.upgrade() else { return };
        let Some(tray) = item_rc.downcast::<super::SystemTray>() else { return };
        let tray = tray.as_pin_ref();
        let menu_borrow = tray.data.menu.borrow();
        let Some(state) = menu_borrow.as_ref() else { return };
        if let Some(entry) = state.entries.get(entry_index) {
            vtable::VRc::borrow(&state.menu_vrc).activate(entry);
        }
    }

    fn activated(&self) {
        let Some(item_rc) = self.self_weak.upgrade() else { return };
        let Some(tray) = item_rc.downcast::<super::SystemTray>() else { return };
        tray.as_pin_ref().activated.call(&());
    }
}

// ---------------------------------------------------------------------------
// PlatformTray: the public backend type, owned by the Slint `SystemTray` item.
// ---------------------------------------------------------------------------

pub struct PlatformTray {
    // Heap-allocated so the address we stash in `GWLP_USERDATA` is stable while
    // `PlatformTray` is alive; moving the `Box` does not relocate the `Inner`.
    inner: std::boxed::Box<Inner>,
}

impl PlatformTray {
    pub fn new(
        params: Params,
        self_weak: ItemWeak,
        _context: &crate::SlintContext,
    ) -> Result<Self, Error> {
        ensure_class_registered()?;
        // Ensure the TaskbarCreated message id is registered before the window can
        // receive it.
        let _ = taskbar_created_message();

        let hinstance = unsafe { GetModuleHandleW(None) }.map_err(|e| {
            Error::PlatformError(crate::platform::PlatformError::Other(e.message()))
        })?;

        // A hidden message-only window: invisible to the user, its only job is to
        // receive `Shell_NotifyIcon` callbacks (`WM_TRAYICON`) and the
        // `TaskbarCreated` broadcast. Wrapped in a scope guard so any `?` or
        // early return below destroys it; defused by `into_inner` on success.
        let hwnd_guard = scopeguard::guard(
            unsafe {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    CLASS_NAME,
                    PCWSTR::null(),
                    WINDOW_STYLE(0),
                    0,
                    0,
                    0,
                    0,
                    Some(HWND_MESSAGE),
                    None,
                    Some(hinstance.into()),
                    None,
                )
            }
            .map_err(|e| {
                Error::PlatformError(crate::platform::PlatformError::Other(e.message()))
            })?,
            |h| unsafe {
                let _ = DestroyWindow(h);
            },
        );

        let hicon_guard = scopeguard::guard(create_hicon(params.icon)?, |h| unsafe {
            let _ = DestroyIcon(h);
        });

        let tip = HSTRING::from(params.title);
        let data = notify_icon_data(*hwnd_guard, *hicon_guard, &tip);
        let added = unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
        if !added.as_bool() {
            return Err(Error::PlatformError(crate::platform::PlatformError::Other(
                "Shell_NotifyIconW(NIM_ADD) failed".into(),
            )));
        }

        // NIM_ADD succeeded: hand ownership of the handles over to `Inner` by
        // defusing the guards. The wnd proc can't observe `Inner` until we stash
        // its pointer in GWLP_USERDATA below, and no `WM_TRAYICON` can be
        // delivered before that either (the Shell only sends those in response
        // to user input after `NIM_ADD` has returned).
        let inner = std::boxed::Box::new(Inner {
            hwnd: scopeguard::ScopeGuard::into_inner(hwnd_guard),
            hicon: scopeguard::ScopeGuard::into_inner(hicon_guard),
            self_weak,
            hmenu: Cell::new(None),
            tip,
        });
        unsafe { SetWindowLongPtrW(inner.hwnd, GWLP_USERDATA, &*inner as *const Inner as isize) };

        Ok(Self { inner })
    }

    pub fn rebuild_menu(
        &self,
        menu: vtable::VRef<'_, MenuVTable>,
        entries_out: &mut std::vec::Vec<MenuEntry>,
    ) {
        entries_out.clear();
        let Ok(new_menu) = (unsafe { CreatePopupMenu() }) else { return };

        let mut top = SharedVector::<MenuEntry>::default();
        menu.sub_menu(None, &mut top);
        for entry in top.iter() {
            append_menu_entry(new_menu, menu, entry, 0, entries_out);
        }

        // Swap first, then free the old menu, so the wnd proc never sees a
        // dangling handle while the swap is in progress.
        if let Some(old) = self.inner.hmenu.replace(Some(new_menu)) {
            let _ = unsafe { DestroyMenu(old) };
        }
    }

    pub fn set_visible(&self, _visible: bool) {
        // TODO: toggle the notification-area icon (NIM_DELETE on hide,
        // NIM_ADD on show, via Shell_NotifyIconW).
    }

    pub fn set_icon(&self, _icon: &crate::graphics::Image) {
        // TODO: rebuild HICON from the new icon and apply it via
        // Shell_NotifyIconW(NIM_MODIFY) (with NIF_ICON), then DestroyIcon
        // the previous handle once the call returns.
    }

    pub fn set_title(&self, _title: &str) {
        // TODO: update the tooltip via Shell_NotifyIconW(NIM_MODIFY) with
        // NIF_TIP.
    }
}

impl Drop for PlatformTray {
    fn drop(&mut self) {
        // Safety: constructed on and only accessed from the event-loop thread,
        // which is the same thread running Drop. The `Box<Inner>` field drops
        // after this body, freeing the heap allocation only once the wnd proc
        // can no longer reach it (USERDATA cleared, HWND destroyed).
        unsafe {
            // NIM_DELETE only needs the identity fields; leave everything else zero.
            let data = NOTIFYICONDATAW {
                cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.inner.hwnd,
                uID: TRAY_UID,
                ..Default::default()
            };
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);

            // Detach from the window before destroying it so any pending messages
            // resolve through DefWindowProc.
            SetWindowLongPtrW(self.inner.hwnd, GWLP_USERDATA, 0);
            if let Some(m) = self.inner.hmenu.take() {
                let _ = DestroyMenu(m);
            }
            let _ = DestroyWindow(self.inner.hwnd);
            let _ = DestroyIcon(self.inner.hicon);
        }
    }
}

// ---------------------------------------------------------------------------
// Window procedure.
// ---------------------------------------------------------------------------

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_TRAYICON {
        // With the default notify-icon version the mouse event is packed in the
        // low word of lparam.
        let event = (lparam.0 as u32) & 0xFFFF;
        if event == WM_RBUTTONUP || event == WM_CONTEXTMENU {
            let inner_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Inner;
            if !inner_ptr.is_null() {
                show_popup_menu(hwnd, unsafe { &*inner_ptr });
            }
        } else if event == WM_LBUTTONUP {
            let inner_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Inner;
            if !inner_ptr.is_null() {
                unsafe { &*inner_ptr }.activated();
            }
        }
        return LRESULT(0);
    }

    if msg == taskbar_created_message() && msg != 0 {
        let inner_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const Inner;
        if !inner_ptr.is_null() {
            let inner = unsafe { &*inner_ptr };
            let data = notify_icon_data(hwnd, inner.hicon, &inner.tip);
            let _ = unsafe { Shell_NotifyIconW(NIM_ADD, &data) };
        }
        return LRESULT(0);
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn show_popup_menu(hwnd: HWND, inner: &Inner) {
    let Some(hmenu) = inner.hmenu.get() else { return };
    let mut pt = POINT { x: 0, y: 0 };
    if unsafe { GetCursorPos(&mut pt) }.is_err() {
        return;
    }
    // Required so the menu dismisses when the user clicks elsewhere (Microsoft
    // knowledge-base article Q135788).
    let _ = unsafe { SetForegroundWindow(hwnd) };
    let cmd = unsafe {
        TrackPopupMenu(
            hmenu,
            TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            None,
            hwnd,
            None,
        )
    };
    let _ = unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) };
    let cmd = cmd.0 as u32;
    if cmd >= MENU_CMD_BASE {
        inner.activate((cmd - MENU_CMD_BASE) as usize);
    }
}

// ===========================================================================
// Helpers below this line: window-class registration, UTF-16 conversion,
// icon conversion, `Shell_NotifyIcon` data plumbing, menu building.
// ===========================================================================

const CLASS_NAME: PCWSTR = w!("SlintSystemTrayWindow");

fn ensure_class_registered() -> Result<(), Error> {
    static REGISTERED: AtomicBool = AtomicBool::new(false);
    if REGISTERED.load(Ordering::Acquire) {
        return Ok(());
    }
    let hinstance = unsafe { GetModuleHandleW(None) }
        .map_err(|e| Error::PlatformError(crate::platform::PlatformError::Other(e.message())))?;
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: CLASS_NAME,
        ..Default::default()
    };
    // Ignore the return value: a non-zero atom is success, and the only realistic
    // failure here is "class already exists" if two copies of this module ended
    // up linked into the same process — in which case `CreateWindowExW` will
    // succeed anyway. A genuinely broken registration will surface as a
    // `CreateWindowExW` failure instead.
    let _ = unsafe { RegisterClassW(&wc) };
    REGISTERED.store(true, Ordering::Release);
    Ok(())
}

// The "TaskbarCreated" message is broadcast to all top-level windows when
// explorer restarts; we use it to re-add our icon. `RegisterWindowMessageW` is
// idempotent, so caching the id is just an optimization.
fn taskbar_created_message() -> u32 {
    static MSG: AtomicU32 = AtomicU32::new(0);
    let cached = MSG.load(Ordering::Relaxed);
    if cached != 0 {
        return cached;
    }
    let id = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
    MSG.store(id, Ordering::Relaxed);
    id
}

// Slint `Image` -> `HICON` via a top-down 32bpp BGRA DIB.
fn create_hicon(icon: &Image) -> Result<HICON, Error> {
    let pixel_buffer = icon.to_rgba8().ok_or(Error::Rgba8)?;
    let width = pixel_buffer.width() as i32;
    let height = pixel_buffer.height() as i32;
    let rgba = pixel_buffer.as_bytes();

    let bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let hdc = unsafe { GetDC(None) };
    let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
    let color_bitmap =
        unsafe { CreateDIBSection(Some(hdc), &bi, DIB_RGB_COLORS, &mut bits, None, 0) };
    let _ = unsafe { ReleaseDC(None, hdc) };
    // `CreateIconIndirect` copies the bitmap data into its own storage, so we
    // always want to delete these at the end — guards fire on the success and
    // error paths both.
    let color_bitmap = scopeguard::guard(color_bitmap.map_err(|_| Error::Rgba8)?, |b| unsafe {
        let _ = DeleteObject(b.into());
    });
    if bits.is_null() {
        return Err(Error::Rgba8);
    }

    let pixel_count = (width * height) as usize;
    let dest = unsafe { std::slice::from_raw_parts_mut(bits as *mut u8, pixel_count * 4) };
    for i in 0..pixel_count {
        let src = i * 4;
        // RGBA -> BGRA.
        dest[src] = rgba[src + 2];
        dest[src + 1] = rgba[src + 1];
        dest[src + 2] = rgba[src];
        dest[src + 3] = rgba[src + 3];
    }

    // Required even though it's effectively ignored for 32bpp-alpha icons.
    let mask = scopeguard::guard(unsafe { CreateBitmap(width, height, 1, 1, None) }, |b| unsafe {
        let _ = DeleteObject(b.into());
    });

    let ii = ICONINFO {
        fIcon: windows::core::BOOL(1),
        xHotspot: 0,
        yHotspot: 0,
        hbmMask: *mask,
        hbmColor: *color_bitmap,
    };

    unsafe { CreateIconIndirect(&ii) }
        .map_err(|e| Error::PlatformError(crate::platform::PlatformError::Other(e.message())))
}

// Fully populated `NOTIFYICONDATAW` for `NIM_ADD` / `NIM_MODIFY`. The unused
// trailing fields (`szInfo`, `szInfoTitle`, `guidItem`, …) are zero-initialized
// via `Default::default()`, which also NUL-terminates `szTip` past the copy.
fn notify_icon_data(hwnd: HWND, hicon: HICON, tip: &[u16]) -> NOTIFYICONDATAW {
    let mut data = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_UID,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: hicon,
        ..Default::default()
    };
    let n = tip.len().min(data.szTip.len() - 1);
    data.szTip[..n].copy_from_slice(&tip[..n]);
    data
}

fn append_menu_entry(
    parent: HMENU,
    menu: vtable::VRef<'_, MenuVTable>,
    entry: &MenuEntry,
    depth: usize,
    entries_out: &mut std::vec::Vec<MenuEntry>,
) {
    if entry.is_separator {
        let _ = unsafe { AppendMenuW(parent, MF_SEPARATOR, 0, PCWSTR::null()) };
        return;
    }

    let title = HSTRING::from(entry.title.as_str());

    if entry.has_sub_menu && depth < MAX_DEPTH {
        let Ok(sub) = (unsafe { CreatePopupMenu() }) else { return };
        let mut children = SharedVector::<MenuEntry>::default();
        menu.sub_menu(Some(entry), &mut children);
        for child in children.iter() {
            append_menu_entry(sub, menu, child, depth + 1, entries_out);
        }
        let mut flags = MF_POPUP | MF_STRING;
        if !entry.enabled {
            flags |= MF_GRAYED | MF_DISABLED;
        }
        let _ = unsafe { AppendMenuW(parent, flags, sub.0 as usize, &title) };
    } else {
        let entry_index = entries_out.len();
        entries_out.push(entry.clone());
        let cmd = MENU_CMD_BASE + entry_index as u32;
        let mut flags = MF_STRING;
        if !entry.enabled {
            flags |= MF_GRAYED | MF_DISABLED;
        }
        let _ = unsafe { AppendMenuW(parent, flags, cmd as usize, &title) };
    }
}
