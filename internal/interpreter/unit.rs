// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The interpreter's per-document storage.

use i_slint_compiler::llr;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// A lowered document together with everything the interpreter derives from it
/// that is the same for all instances of a component.
///
/// Instances keep this alive (through their sub-components), so anything cached
/// here outlives every instance that reads it — which is what lets those
/// derived tables be shared rather than rebuilt per instance, and what will let
/// a runtime-built item tree description live here too.
pub struct InterpreterUnit {
    llr: llr::CompilationUnit,
    /// Keyed by the address of the `llr::ItemTree` the tables were built from,
    /// which is stable because this owns the compilation unit.
    item_tree_tables: RefCell<HashMap<*const llr::ItemTree, Rc<super::instance::ItemTreeTables>>>,
}

impl InterpreterUnit {
    pub fn new(llr: llr::CompilationUnit) -> Self {
        Self { llr, item_tree_tables: RefCell::new(HashMap::new()) }
    }

    /// The flattened item tree tables for `item_tree`, built on first use.
    ///
    /// They are a pure function of the lowered tree, so every instance of the
    /// component shares one copy.
    pub fn item_tree_tables(
        &self,
        item_tree: &llr::ItemTree,
        build: impl FnOnce() -> super::instance::ItemTreeTables,
    ) -> Rc<super::instance::ItemTreeTables> {
        let key = item_tree as *const llr::ItemTree;
        if let Some(tables) = self.item_tree_tables.borrow().get(&key) {
            return tables.clone();
        }
        // Build outside the borrow: `build` walks the compilation unit.
        let tables = Rc::new(build());
        self.item_tree_tables.borrow_mut().insert(key, tables.clone());
        tables
    }
}

impl std::ops::Deref for InterpreterUnit {
    type Target = llr::CompilationUnit;
    fn deref(&self) -> &Self::Target {
        &self.llr
    }
}

impl std::fmt::Debug for InterpreterUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.llr.fmt(f)
    }
}
