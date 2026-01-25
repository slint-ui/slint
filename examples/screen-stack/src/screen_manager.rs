// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! Screen manager - handles navigation stack with lazy controller creation.

use crate::screen::{Screen, ScreenContext, ScreenFactory};
use i_slint_core::component_factory::FactoryContext;
use slint::ComponentFactory;
use slint_interpreter::{Compiler, ComponentDefinition, ComponentHandle, ComponentInstance};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

/// Entry in the screen stack.
struct StackEntry {
    name: String,
    controller: Rc<dyn Screen>,
    /// Shared reference to the instance, set by ComponentFactory
    instance: Rc<RefCell<Option<ComponentInstance>>>,
}

/// Manages the screen stack with lazy loading of controllers.
pub struct ScreenManager {
    definitions: HashMap<String, ComponentDefinition>,
    stack: Vec<StackEntry>,
    factories: HashMap<String, ScreenFactory>,
    screens_path: PathBuf,
    ctx: ScreenContext,
    /// Callback: (factory, depth, is_pushing)
    update_ui: Rc<RefCell<Option<Box<dyn Fn(ComponentFactory, i32, bool)>>>>,
}

impl ScreenManager {
    pub fn new(screens_path: PathBuf) -> Rc<RefCell<Self>> {
        let manager = Rc::new(RefCell::new(Self {
            definitions: HashMap::new(),
            stack: Vec::new(),
            factories: HashMap::new(),
            screens_path,
            ctx: ScreenContext::new(),
            update_ui: Rc::new(RefCell::new(None)),
        }));

        {
            let mgr1 = manager.clone();
            let mgr2 = manager.clone();
            let mgr3 = manager.clone();
            let mgr4 = manager.clone();
            let mgr5 = manager.clone();
            manager.borrow().ctx.set_navigation(
                move |name| mgr1.borrow_mut().push(name),
                move || mgr2.borrow_mut().pop(),
                move |name| mgr3.borrow_mut().replace(name),
                move |name| mgr4.borrow_mut().clear(name),
                move || mgr5.borrow_mut().pop_to_root(),
            );
        }

        manager
    }

    pub fn set_ui_callback(&mut self, cb: impl Fn(ComponentFactory, i32, bool) + 'static) {
        *self.update_ui.borrow_mut() = Some(Box::new(cb));
    }

    pub fn register(&mut self, name: &str, factory: impl Fn() -> Box<dyn Screen> + 'static) {
        self.factories.insert(name.to_string(), Box::new(factory));
    }

    pub fn push(&mut self, name: &str) {
        println!("\n>>> PUSH: {}", name);

        let factory =
            self.factories.get(name).unwrap_or_else(|| panic!("Screen '{}' not registered", name));
        let controller: Rc<dyn Screen> = Rc::from(factory());

        self.stack.push(StackEntry {
            name: name.to_string(),
            controller,
            instance: Rc::new(RefCell::new(None)),
        });

        self.show_current(true); // is_pushing = true
    }

    pub fn pop(&mut self) {
        if self.stack.len() <= 1 {
            println!(">>> POP: Cannot pop root screen");
            return;
        }

        println!("\n>>> POP");

        // Call on_unload before destroying
        if let Some(entry) = self.stack.pop() {
            if let Some(instance) = entry.instance.borrow().as_ref() {
                entry.controller.on_unload(instance);
            }
            println!(">>> Stack depth now: {}", self.stack.len());
            // Controller dropped here
        }

        self.show_current(false); // is_pushing = false
    }

    /// Replace current screen with a new one (pop + push without going back)
    pub fn replace(&mut self, name: &str) {
        println!(
            "\n>>> REPLACE: {} -> {}",
            self.stack.last().map(|e| e.name.as_str()).unwrap_or("?"),
            name
        );

        // Pop current without showing previous
        if let Some(entry) = self.stack.pop() {
            if let Some(instance) = entry.instance.borrow().as_ref() {
                entry.controller.on_unload(instance);
            }
        }

        // Push new screen
        let factory =
            self.factories.get(name).unwrap_or_else(|| panic!("Screen '{}' not registered", name));
        let controller: Rc<dyn Screen> = Rc::from(factory());

        self.stack.push(StackEntry {
            name: name.to_string(),
            controller,
            instance: Rc::new(RefCell::new(None)),
        });

        self.show_current(true); // animate as push
    }

    /// Clear entire stack and push new root screen
    pub fn clear(&mut self, root_name: &str) {
        println!("\n>>> CLEAR: reset to {}", root_name);

        // Unload all screens
        while let Some(entry) = self.stack.pop() {
            if let Some(instance) = entry.instance.borrow().as_ref() {
                entry.controller.on_unload(instance);
            }
        }

        // Push new root
        let factory = self
            .factories
            .get(root_name)
            .unwrap_or_else(|| panic!("Screen '{}' not registered", root_name));
        let controller: Rc<dyn Screen> = Rc::from(factory());

        self.stack.push(StackEntry {
            name: root_name.to_string(),
            controller,
            instance: Rc::new(RefCell::new(None)),
        });

        self.show_current(true);
    }

    /// Pop all screens except root
    pub fn pop_to_root(&mut self) {
        if self.stack.len() <= 1 {
            println!(">>> POP_TO_ROOT: Already at root");
            return;
        }

        println!("\n>>> POP_TO_ROOT");

        // Unload all except first
        while self.stack.len() > 1 {
            if let Some(entry) = self.stack.pop() {
                if let Some(instance) = entry.instance.borrow().as_ref() {
                    entry.controller.on_unload(instance);
                }
            }
        }

        println!(">>> Stack depth now: {}", self.stack.len());
        self.show_current(false);
    }

    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    fn show_current(&mut self, is_pushing: bool) {
        let Some(entry) = self.stack.last() else { return };
        let name = entry.name.clone();
        let controller = entry.controller.clone();
        let instance_holder = entry.instance.clone();
        let ctx = self.ctx.clone();

        // Use template if specified, otherwise screen-specific .slint
        let definition = match controller.template() {
            Some(template) => self.get_or_compile_template(template),
            None => self.get_or_compile_screen(&name),
        };

        // Clone ctx for go-back callback
        let ctx_for_back = ctx.clone();

        let factory = ComponentFactory::new(move |fctx: FactoryContext| {
            let instance = definition.create_embedded(fctx).ok()?;

            // Set up go-back callback (from ScreenWithHeader base)
            let ctx_back = ctx_for_back.clone();
            let _ = instance.set_callback("go-back", move |_| {
                ctx_back.pop();
                slint_interpreter::Value::Void
            });

            // Set up controller-specific callbacks
            controller.on_loaded(&instance, &ctx);

            // Store instance for later use in on_unload
            *instance_holder.borrow_mut() = Some(instance.clone_strong());

            Some(instance)
        });

        let depth = self.stack.len() as i32;
        if let Some(cb) = self.update_ui.borrow().as_ref() {
            cb(factory, depth, is_pushing);
        }
    }

    /// Compile a screen-specific .slint file (e.g., screens/home.slint)
    fn get_or_compile_screen(&mut self, name: &str) -> ComponentDefinition {
        let cache_key = format!("screen:{}", name);
        if let Some(def) = self.definitions.get(&cache_key) {
            return def.clone();
        }

        let path = self.screens_path.join(format!("{}.slint", name));
        let component_name = to_component_name(name);

        let def = self.compile(&path, &component_name);
        self.definitions.insert(cache_key, def.clone());
        def
    }

    /// Compile a template component (e.g., components/list-screen.slint)
    fn get_or_compile_template(&mut self, template: &str) -> ComponentDefinition {
        let cache_key = format!("template:{}", template);
        if let Some(def) = self.definitions.get(&cache_key) {
            return def.clone();
        }

        let components_path = self
            .screens_path
            .parent()
            .map(|p| p.join("components"))
            .unwrap_or_else(|| self.screens_path.join("../components"));
        let path = components_path.join(format!("{}.slint", template));
        let component_name = to_component_name(template);

        let def = self.compile(&path, &component_name);
        self.definitions.insert(cache_key, def.clone());
        def
    }

    fn compile(&self, path: &PathBuf, component_name: &str) -> ComponentDefinition {
        println!("Compiling: {:?}", path);

        let mut compiler = Compiler::new();
        let components_path = self
            .screens_path
            .parent()
            .map(|p| p.join("components"))
            .unwrap_or_else(|| self.screens_path.join("../components"));
        compiler.set_include_paths(vec![self.screens_path.clone(), components_path]);

        let result = futures::executor::block_on(compiler.build_from_path(path));

        if result.has_errors() {
            for diag in result.diagnostics() {
                eprintln!("{}", diag);
            }
            panic!("Failed to compile: {:?}", path);
        }

        result
            .component(component_name)
            .unwrap_or_else(|| panic!("Component {} not found in {:?}", component_name, path))
    }
}

/// Convert screen/template name to component name
/// For templates: "list-screen" -> "ListScreen" (PascalCase)
/// For screens: "contact-detail" -> "Contact-detailScreen" (matches .slint convention)
fn to_component_name(name: &str) -> String {
    // Templates use full PascalCase (ListScreen, TabView)
    if name.ends_with("-screen") || name.ends_with("-view") {
        let mut result = String::new();
        let mut capitalize_next = true;
        for ch in name.chars() {
            if ch == '-' {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(ch.to_ascii_uppercase());
                capitalize_next = false;
            } else {
                result.push(ch);
            }
        }
        return result;
    }

    // Screens: capitalize first letter, keep hyphens, add "Screen"
    // e.g., "contact-detail" -> "Contact-detailScreen"
    let mut result = String::new();
    let mut chars = name.chars();
    if let Some(first) = chars.next() {
        result.push(first.to_ascii_uppercase());
    }
    result.extend(chars);
    result.push_str("Screen");
    result
}
