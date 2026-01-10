// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Provides a mechanism to defer initialization tasks until after the current
//! initialization scope completes. This is used to break recursion cycles when
//! change trackers evaluate properties that trigger layout computation, which
//! in turn creates new components with their own change trackers.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::RefCell; // Used for PENDING_INITIALIZATIONS

crate::thread_local! {
    /// Holds pending initializations.
    /// None = No active scope (run immediate).
    /// Some(Vec) = Active scope (queue tasks).
    static PENDING_INITIALIZATIONS: RefCell<Option<Vec<Box<dyn FnOnce()>>>> = RefCell::new(None)
}

/// Runs the given closure `f`. Any `defer_initialization` calls made within `f`
/// (even recursively) will be queued and executed only after `f` completes.
///
/// If already inside an initialization scope, this simply runs `f` directly
/// (the outer scope will process all queued tasks).
pub fn with_initialization_scope<R>(f: impl FnOnce() -> R) -> R {
    // Check if we are already in a scope
    if PENDING_INITIALIZATIONS.with(|q| q.borrow().is_some()) {
        return f();
    }

    // Start a new scope
    PENDING_INITIALIZATIONS.with(|q| *q.borrow_mut() = Some(Vec::new()));

    // Safety guard: Ensure we clear the scope even if f() panics
    struct ScopeGuard;
    impl Drop for ScopeGuard {
        fn drop(&mut self) {
            PENDING_INITIALIZATIONS.with(|q| *q.borrow_mut() = None);
        }
    }
    let _guard = ScopeGuard;

    let result = f();

    // Process the queue
    loop {
        let batch = PENDING_INITIALIZATIONS.with(|q| {
            // Take the current batch of tasks
            let mut borrow = q.borrow_mut();
            let vec = borrow.as_mut().unwrap();
            if vec.is_empty() { None } else { Some(core::mem::take(vec)) }
        });

        match batch {
            // Run the batch. Note: running these might queue NEW tasks
            // (e.g. if they trigger layout -> ensure_updated -> nested startup),
            // which will be caught by the next iteration of the loop.
            Some(tasks) => {
                for task in tasks {
                    task();
                }
            }
            None => break,
        }
    }

    result
}

/// Queue a task to run at the end of the current initialization scope.
/// If no scope is active, creates a new scope and runs the task immediately.
pub fn defer_initialization(task: impl FnOnce() + 'static) {
    let mut task = Some(task);
    let did_queue = PENDING_INITIALIZATIONS.with(|q| {
        let mut b = q.borrow_mut();
        if let Some(vec) = b.as_mut() {
            if let Some(t) = task.take() {
                vec.push(Box::new(t));
            }
            true
        } else {
            false
        }
    });

    if !did_queue {
        if let Some(t) = task {
            with_initialization_scope(t);
        }
    }
}

/// Returns true if we are currently inside an initialization scope.
pub fn is_in_initialization_scope() -> bool {
    PENDING_INITIALIZATIONS.with(|q| q.borrow().is_some())
}

/// Begin an initialization scope manually. Returns true if a new scope was created,
/// false if we're already inside a scope.
///
/// If this returns true, you MUST call `end_initialization_scope()` to process
/// the deferred tasks and clean up.
///
/// Prefer using `with_initialization_scope()` when possible as it handles cleanup
/// automatically even on panic.
pub fn begin_initialization_scope() -> bool {
    PENDING_INITIALIZATIONS.with(|q| {
        let mut b = q.borrow_mut();
        if b.is_some() {
            false // Already in a scope
        } else {
            *b = Some(Vec::new());
            true // New scope created
        }
    })
}

/// End an initialization scope and process all deferred tasks.
///
/// This should only be called if `begin_initialization_scope()` returned true.
/// Calling it when not in a scope or in a nested scope will have no effect.
pub fn end_initialization_scope() {
    // Process the queue
    loop {
        let batch = PENDING_INITIALIZATIONS.with(|q| {
            let mut borrow = q.borrow_mut();
            if let Some(vec) = borrow.as_mut() {
                if vec.is_empty() { None } else { Some(core::mem::take(vec)) }
            } else {
                None
            }
        });

        match batch {
            Some(tasks) => {
                for task in tasks {
                    task();
                }
            }
            None => break,
        }
    }

    // Clear the scope
    PENDING_INITIALIZATIONS.with(|q| *q.borrow_mut() = None);
}
