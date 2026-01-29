// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Screen Stack Example
//!
//! Demonstrates dynamic screen loading with Model-Controller pattern.
//! Controllers are created when pushed and destroyed when popped,
//! ensuring memory is only used for screens on the stack.
//!
//! State is managed centrally via AppStore, which persists across
//! screen instance lifecycles.

use screen_stack::screen_manager::ScreenManager;
use screen_stack::screens::*;
use screen_stack::store::AppStore;
use std::path::PathBuf;

slint::include_modules!();

fn main() {
    let app = App::new().unwrap();

    // Find screens directory
    let screens_path = find_screens_path();
    println!("Screens path: {:?}", screens_path);

    // Create centralized state store
    let store = AppStore::new();

    // Create screen manager
    let manager = ScreenManager::new(screens_path);

    // Register screen factories - controllers created lazily
    {
        let mut mgr = manager.borrow_mut();

        // Home screen - no persistent state
        let s = store.clone();
        mgr.register("home", move || HomeScreen::create(s.clone()));

        // Settings screens (hierarchical)
        let s = store.clone();
        mgr.register("settings", move || SettingsScreen::create(s.clone()));
        let s = store.clone();
        mgr.register("settings-appearance", move || SettingsAppearanceScreen::create(s.clone()));
        let s = store.clone();
        mgr.register("settings-notifications", move || {
            SettingsNotificationsScreen::create(s.clone())
        });
        let s = store.clone();
        mgr.register("settings-sound", move || SettingsSoundScreen::create(s.clone()));
        let s = store.clone();
        mgr.register("settings-about", move || SettingsAboutScreen::create(s.clone()));

        // Profile screen
        let s = store.clone();
        mgr.register("profile", move || ProfileScreen::create(s.clone()));

        // Advanced screen
        let s = store.clone();
        mgr.register("advanced", move || AdvancedScreen::create(s.clone()));

        // List screen examples
        let s = store.clone();
        mgr.register("contacts", move || ContactsScreen::create(s.clone()));
        let s = store.clone();
        mgr.register("orders", move || OrdersScreen::create(s.clone()));
        let s = store.clone();
        mgr.register("notifications-list", move || NotificationsListScreen::create(s.clone()));

        // Detail screens
        let s = store.clone();
        mgr.register("contact-detail", move || ContactDetailScreen::create(s.clone()));
        let s = store.clone();
        mgr.register("order-detail", move || OrderDetailScreen::create(s.clone()));
        let s = store.clone();
        mgr.register("notification-detail", move || NotificationDetailScreen::create(s.clone()));

        // Tab view screens
        let s = store.clone();
        mgr.register("dashboard", move || DashboardScreen::create(s.clone()));
        let s = store.clone();
        mgr.register("media", move || MediaScreen::create(s.clone()));

        // Set UI update callback with animation (dual-buffer approach)
        let app_weak = app.as_weak();
        let use_b = std::cell::Cell::new(false);
        mgr.set_ui_callback(move |factory, depth, is_pushing| {
            if let Some(app) = app_weak.upgrade() {
                let nav = app.global::<Navigation>();
                // Set direction for animation
                nav.set_is_pushing(is_pushing);
                // Alternate between screen-a and screen-b
                let next_use_b = !use_b.get();
                if next_use_b {
                    nav.set_screen_b(factory);
                } else {
                    nav.set_screen_a(factory);
                }
                nav.set_show_b(next_use_b);
                nav.set_stack_depth(depth);
                use_b.set(next_use_b);
            }
        });
    }

    // Connect Navigation callbacks
    {
        let mgr = manager.clone();
        app.global::<Navigation>().on_push(move |name| {
            mgr.borrow_mut().push(&name);
        });
    }
    {
        let mgr = manager.clone();
        app.global::<Navigation>().on_pop(move || {
            mgr.borrow_mut().pop();
        });
    }
    {
        let mgr = manager.clone();
        app.global::<Navigation>().on_replace(move |name| {
            mgr.borrow_mut().replace(&name);
        });
    }
    {
        let mgr = manager.clone();
        app.global::<Navigation>().on_clear(move |name| {
            mgr.borrow_mut().clear(&name);
        });
    }
    {
        let mgr = manager.clone();
        app.global::<Navigation>().on_pop_to_root(move || {
            mgr.borrow_mut().pop_to_root();
        });
    }

    // Push initial screen
    println!("=== Starting application ===");
    manager.borrow_mut().push("home");

    app.run().unwrap();

    println!("\n=== Application closed ===");
}

fn find_screens_path() -> PathBuf {
    let candidates = [
        std::env::var("CARGO_MANIFEST_DIR").map(|p| PathBuf::from(p).join("ui/screens")).ok(),
        Some(std::env::current_dir().unwrap().join("ui/screens")),
        Some(std::env::current_dir().unwrap().join("examples/screen-stack/ui/screens")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return candidate;
        }
    }

    panic!("Could not find screens directory");
}
