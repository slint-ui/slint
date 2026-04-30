// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! macOS system tray backend using AppKit (`NSStatusBar`, `NSStatusItem`, `NSMenu`).
//!
//! Everything in this module must run on the main thread. `SystemTrayHandle::new` is
//! driven by a `ChangeTracker` that fires from the Slint event loop, which on macOS
//! with the winit backend is the main thread.

use super::{Error, Params};
use crate::SharedVector;
use crate::graphics::Image;
use crate::item_tree::ItemWeak;
use crate::items::MenuEntry;
use crate::menus::MenuVTable;

use objc2::rc::Retained;
use objc2::runtime::Sel;
use objc2::{
    AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
use objc2_app_kit::{
    NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{NSData, NSObject, NSObjectProtocol, NSSize, NSString};

// Mirror the other backends' depth cap to protect against accidental infinite menu trees.
const MAX_DEPTH: usize = 15;

// ---------------------------------------------------------------------------
// MenuAction: the ObjC target object attached to every NSMenuItem. The menu
// item's `tag` carries the entry index; the selector upgrades the weak and
// dispatches activation back through the Slint event loop.
// ---------------------------------------------------------------------------

struct MenuActionIvars {
    self_weak: ItemWeak,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MenuActionIvars]
    struct MenuAction;

    unsafe impl NSObjectProtocol for MenuAction {}

    impl MenuAction {
        #[unsafe(method(activated:))]
        fn activated(&self, sender: &NSMenuItem) {
            let entry_index = sender.tag() as usize;
            // AppKit fires menu actions on the main thread, which with the winit
            // backend is the Slint event-loop thread; dispatch directly.
            activate_entry(&self.ivars().self_weak, entry_index);
        }
    }
);

impl MenuAction {
    fn new(mtm: MainThreadMarker, self_weak: ItemWeak) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MenuActionIvars { self_weak });
        unsafe { msg_send![super(this), init] }
    }
}

fn activate_entry(self_weak: &ItemWeak, entry_index: usize) {
    let Some(item_rc) = self_weak.upgrade() else { return };
    let Some(tray) = item_rc.downcast::<super::SystemTray>() else { return };
    let tray = tray.as_pin_ref();
    let menu_borrow = tray.data.menu.borrow();
    let Some(state) = menu_borrow.as_ref() else { return };
    if let Some(entry) = state.entries.get(entry_index) {
        vtable::VRc::borrow(&state.menu_vrc).activate(entry);
    }
}

// ---------------------------------------------------------------------------
// Icon conversion: Slint `Image` -> `NSImage` via `NSBitmapImageRep`.
// ---------------------------------------------------------------------------

fn image_to_nsimage(icon: &Image) -> Result<Retained<NSImage>, Error> {
    let pixel_buffer = icon.to_rgba8().ok_or(Error::Rgba8)?;
    let width = pixel_buffer.width();
    let height = pixel_buffer.height();

    // PNG-encode via the `image` crate, then hand the bytes to NSImage. This keeps
    // the whole conversion inside safe APIs and avoids the pixel format / alpha
    // premultiplication pitfalls of building an NSBitmapImageRep by hand.
    let mut png = std::vec::Vec::<u8>::new();
    <image::codecs::png::PngEncoder<_> as image::ImageEncoder>::write_image(
        image::codecs::png::PngEncoder::new(&mut png),
        pixel_buffer.as_bytes(),
        width,
        height,
        image::ExtendedColorType::Rgba8,
    )
    .map_err(|_| Error::Rgba8)?;

    let data = NSData::with_bytes(&png);
    let image = NSImage::initWithData(NSImage::alloc(), &data).ok_or(Error::Rgba8)?;

    // macOS status bar icons are conventionally 18pt tall, with width scaled to
    // preserve aspect ratio. The PNG representation keeps full resolution for
    // HiDPI; AppKit scales to this point size.
    const TRAY_ICON_HEIGHT_PT: f64 = 18.0;
    let aspect = width as f64 / height as f64;
    image.setSize(NSSize { width: TRAY_ICON_HEIGHT_PT * aspect, height: TRAY_ICON_HEIGHT_PT });
    Ok(image)
}

// ---------------------------------------------------------------------------
// PlatformTray: one per SystemTray item.
// ---------------------------------------------------------------------------

pub struct PlatformTray {
    status_item: Retained<NSStatusItem>,
    action_target: Retained<MenuAction>,
    mtm: MainThreadMarker,
}

impl PlatformTray {
    pub fn new(
        params: Params,
        self_weak: ItemWeak,
        _context: &crate::SlintContext,
    ) -> Result<Self, Error> {
        let mtm = MainThreadMarker::new()
            .expect("SystemTray must be created on the main thread on macOS");

        let image = image_to_nsimage(params.icon)?;

        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSVariableStatusItemLength);

        if let Some(button) = status_item.button(mtm) {
            button.setImage(Some(&image));
            let tooltip = NSString::from_str(params.title);
            button.setToolTip(Some(&tooltip));
        }

        let action_target = MenuAction::new(mtm, self_weak);

        Ok(Self { status_item, action_target, mtm })
    }

    pub fn rebuild_menu(
        &self,
        menu: vtable::VRef<'_, MenuVTable>,
        entries_out: &mut std::vec::Vec<MenuEntry>,
    ) {
        entries_out.clear();
        let ns_menu = build_menu(menu, &self.action_target, self.mtm, entries_out);
        self.status_item.setMenu(Some(&ns_menu));
    }

    pub fn set_visible(&self, visible: bool) {
        // NSStatusItem has a real visibility toggle since macOS 10.12, so unlike
        // the SNI/ksni backend we don't need to deregister and respawn.
        self.status_item.setVisible(visible);
    }

    pub fn set_icon(&self, _icon: &crate::graphics::Image) {
        // TODO: rebuild NSImage from the new icon and call
        // `button.setImage(...)` on the status item button.
    }

    pub fn set_title(&self, _title: &str) {
        // TODO: update the NSStatusItem button's tool tip via
        // `button.setToolTip(...)`.
    }
}

impl Drop for PlatformTray {
    fn drop(&mut self) {
        // Safe: PlatformTray is only constructed on the main thread and is owned by
        // the SystemTray item, which is itself dropped on the event loop (main thread).
        let status_bar = NSStatusBar::systemStatusBar();
        status_bar.removeStatusItem(&self.status_item);
    }
}

// ---------------------------------------------------------------------------
// Menu building.
// ---------------------------------------------------------------------------

fn build_menu(
    menu: vtable::VRef<'_, MenuVTable>,
    target: &MenuAction,
    mtm: MainThreadMarker,
    entries_out: &mut std::vec::Vec<MenuEntry>,
) -> Retained<NSMenu> {
    let ns_menu = NSMenu::new(mtm);
    let mut top = SharedVector::<MenuEntry>::default();
    menu.sub_menu(None, &mut top);
    for entry in top.iter() {
        let item = entry_to_nsmenuitem(menu, entry, 0, target, mtm, entries_out);
        ns_menu.addItem(&item);
    }
    ns_menu
}

fn entry_to_nsmenuitem(
    menu: vtable::VRef<'_, MenuVTable>,
    entry: &MenuEntry,
    depth: usize,
    target: &MenuAction,
    mtm: MainThreadMarker,
    entries_out: &mut std::vec::Vec<MenuEntry>,
) -> Retained<NSMenuItem> {
    if entry.is_separator {
        return NSMenuItem::separatorItem(mtm);
    }

    if entry.has_sub_menu && depth < MAX_DEPTH {
        // Submenu headers aren't clicked directly, so no target/action/tag is needed.
        let title = NSString::from_str(entry.title.as_str());
        let item = NSMenuItem::new(mtm);
        item.setTitle(&title);
        item.setEnabled(entry.enabled);
        let sub_ns_menu = NSMenu::new(mtm);
        sub_ns_menu.setTitle(&title);
        let mut children = SharedVector::<MenuEntry>::default();
        menu.sub_menu(Some(entry), &mut children);
        for child in children.iter() {
            let child_item = entry_to_nsmenuitem(menu, child, depth + 1, target, mtm, entries_out);
            sub_ns_menu.addItem(&child_item);
        }
        item.setSubmenu(Some(&sub_ns_menu));
        item
    } else {
        let entry_index = entries_out.len();
        entries_out.push(entry.clone());

        let title = NSString::from_str(entry.title.as_str());
        let item = NSMenuItem::new(mtm);
        item.setTitle(&title);
        item.setEnabled(entry.enabled);
        item.setTag(entry_index as isize);
        // Tray menu items are never targeted at a focused responder; wire the action
        // to our MenuAction object directly. `shortcut` bindings on MenuItem are
        // ignored (see the SystemTray docs) so no key equivalent is set.
        unsafe { item.setTarget(Some(target.as_ref())) };
        let action: Sel = sel!(activated:);
        unsafe { item.setAction(Some(action)) };
        item
    }
}
