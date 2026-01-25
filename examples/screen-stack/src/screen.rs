// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Screen trait and context definitions.

use slint_interpreter::ComponentInstance;
use std::cell::RefCell;
use std::rc::Rc;

/// Context passed to screens for navigation.
#[derive(Clone)]
pub struct ScreenContext {
    push_fn: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    pop_fn: Rc<RefCell<Option<Box<dyn Fn()>>>>,
    replace_fn: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    clear_fn: Rc<RefCell<Option<Box<dyn Fn(&str)>>>>,
    pop_to_root_fn: Rc<RefCell<Option<Box<dyn Fn()>>>>,
}

impl ScreenContext {
    pub fn new() -> Self {
        Self {
            push_fn: Rc::new(RefCell::new(None)),
            pop_fn: Rc::new(RefCell::new(None)),
            replace_fn: Rc::new(RefCell::new(None)),
            clear_fn: Rc::new(RefCell::new(None)),
            pop_to_root_fn: Rc::new(RefCell::new(None)),
        }
    }

    pub fn set_navigation(
        &self,
        push: impl Fn(&str) + 'static,
        pop: impl Fn() + 'static,
        replace: impl Fn(&str) + 'static,
        clear: impl Fn(&str) + 'static,
        pop_to_root: impl Fn() + 'static,
    ) {
        *self.push_fn.borrow_mut() = Some(Box::new(push));
        *self.pop_fn.borrow_mut() = Some(Box::new(pop));
        *self.replace_fn.borrow_mut() = Some(Box::new(replace));
        *self.clear_fn.borrow_mut() = Some(Box::new(clear));
        *self.pop_to_root_fn.borrow_mut() = Some(Box::new(pop_to_root));
    }

    /// Push a new screen onto the stack
    pub fn push(&self, screen_name: &str) {
        if let Some(f) = self.push_fn.borrow().as_ref() {
            f(screen_name);
        }
    }

    /// Pop the current screen
    pub fn pop(&self) {
        if let Some(f) = self.pop_fn.borrow().as_ref() {
            f();
        }
    }

    /// Replace current screen with a new one
    pub fn replace(&self, screen_name: &str) {
        if let Some(f) = self.replace_fn.borrow().as_ref() {
            f(screen_name);
        }
    }

    /// Clear stack and set new root screen
    pub fn clear(&self, root_name: &str) {
        if let Some(f) = self.clear_fn.borrow().as_ref() {
            f(root_name);
        }
    }

    /// Pop all screens, return to root
    pub fn pop_to_root(&self) {
        if let Some(f) = self.pop_to_root_fn.borrow().as_ref() {
            f();
        }
    }
}

/// Trait that all screen controllers must implement.
///
/// Controllers are instantiated when pushed onto the stack and
/// destroyed when popped. This ensures memory is only used for
/// screens currently in the navigation stack.
pub trait Screen {
    /// Returns the screen's identifier.
    fn name(&self) -> &'static str;

    /// Returns the template/component to use.
    /// - If None, uses `screens/{name}.slint` (custom screen)
    /// - If Some("list-screen"), uses `components/list-screen.slint` directly
    fn template(&self) -> Option<&'static str> {
        None // Default: use screen-specific .slint file
    }

    /// Called when the screen is loaded.
    fn on_loaded(&self, instance: &ComponentInstance, ctx: &ScreenContext);

    /// Called when the screen is about to be destroyed (popped).
    fn on_unload(&self, _instance: &ComponentInstance) {}

    /// Called when returning to this screen.
    fn on_resume(&self, _instance: &ComponentInstance) {}

    /// Called when covered by another screen.
    fn on_pause(&self, _instance: &ComponentInstance) {}
}

/// Factory function type for creating screen controllers.
/// Called each time a screen is pushed onto the stack.
pub type ScreenFactory = Box<dyn Fn() -> Box<dyn Screen>>;

// ============================================================================
// Screen Type Hierarchy (mirrors the .slint component hierarchy)
// ============================================================================

/// Extension trait for screens using ScreenWithHeader base component.
/// Provides typed helpers for header-related functionality.
pub trait ScreenWithHeader: Screen {
    /// Title displayed in the header. Override to customize.
    fn title(&self) -> &str {
        self.name()
    }

    /// Whether the back button should be shown. Default: true.
    fn can_go_back(&self) -> bool {
        true
    }
}

/// Extension trait for screens using TabScreen base component.
pub trait TabScreen: ScreenWithHeader {
    /// Tab labels.
    fn tabs(&self) -> Vec<String>;

    /// Called when the user switches tabs.
    fn on_tab_changed(&self, _index: i32) {}
}

// ============================================================================
// Helpers for setting up callbacks
// ============================================================================

use slint::SharedString;
use slint_interpreter::Value;

/// Helper to set up common callbacks for any screen.
pub struct ScreenCallbackHelper;

impl ScreenCallbackHelper {
    /// Sets up the navigate callback (push).
    pub fn setup_navigate(instance: &ComponentInstance, ctx: &ScreenContext) {
        let ctx = ctx.clone();
        let _ = instance.set_callback("navigate", move |args| {
            if let Some(Value::String(name)) = args.first() {
                ctx.push(name.as_str());
            }
            Value::Void
        });
    }

    /// Sets up the replace callback.
    pub fn setup_replace(instance: &ComponentInstance, ctx: &ScreenContext) {
        let ctx = ctx.clone();
        let _ = instance.set_callback("replace", move |args| {
            if let Some(Value::String(name)) = args.first() {
                ctx.replace(name.as_str());
            }
            Value::Void
        });
    }

    /// Sets up the clear callback.
    pub fn setup_clear(instance: &ComponentInstance, ctx: &ScreenContext) {
        let ctx = ctx.clone();
        let _ = instance.set_callback("clear", move |args| {
            if let Some(Value::String(name)) = args.first() {
                ctx.clear(name.as_str());
            }
            Value::Void
        });
    }

    /// Sets up the pop-to-root callback.
    pub fn setup_pop_to_root(instance: &ComponentInstance, ctx: &ScreenContext) {
        let ctx = ctx.clone();
        let _ = instance.set_callback("pop-to-root", move |_| {
            ctx.pop_to_root();
            Value::Void
        });
    }

    /// Sets up title property from a Screen trait.
    pub fn setup_title(instance: &ComponentInstance, title: &str) {
        let _ = instance.set_property("title", Value::String(SharedString::from(title)));
    }

    /// Sets up tab-changed callback for TabScreen.
    pub fn setup_tab_changed<F>(instance: &ComponentInstance, callback: F)
    where
        F: Fn(i32) + 'static,
    {
        let _ = instance.set_callback("tab-changed", move |args| {
            if let Some(Value::Number(index)) = args.first() {
                callback(*index as i32);
            }
            Value::Void
        });
    }
}
