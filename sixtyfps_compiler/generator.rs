/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */
/*!
The module responsible for the code generation.

There is one sub module for every language
*/

use crate::diagnostics::BuildDiagnostics;
use crate::object_tree::{Component, Document, ElementRc};

#[cfg(feature = "cpp")]
mod cpp;

#[cfg(feature = "rust")]
pub mod rust;

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum OutputFormat {
    #[cfg(feature = "cpp")]
    Cpp,
    #[cfg(feature = "rust")]
    Rust,
}

impl OutputFormat {
    pub fn guess_from_extension(path: &std::path::Path) -> Option<Self> {
        match path.extension().and_then(|ext| ext.to_str()) {
            #[cfg(feature = "cpp")]
            Some("cpp") | Some("cxx") | Some("h") | Some("hpp") => Some(Self::Cpp),
            #[cfg(feature = "rust")]
            Some("rs") => Some(Self::Rust),
            _ => None,
        }
    }
}

impl std::str::FromStr for OutputFormat {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            #[cfg(feature = "cpp")]
            "cpp" => Ok(Self::Cpp),
            #[cfg(feature = "rust")]
            "rust" => Ok(Self::Rust),
            _ => Err(format!("Unknown outpout format {}", s)),
        }
    }
}

pub fn generate(
    format: OutputFormat,
    destination: &mut impl std::io::Write,
    doc: &Document,
    diag: &mut BuildDiagnostics,
) -> std::io::Result<()> {
    #![allow(unused_variables)]
    #![allow(unreachable_code)]
    match format {
        #[cfg(feature = "cpp")]
        OutputFormat::Cpp => {
            if let Some(output) = cpp::generate(doc, diag) {
                write!(destination, "{}", output)?;
            }
        }
        #[cfg(feature = "rust")]
        OutputFormat::Rust => {
            if let Some(output) = rust::generate(doc, diag) {
                write!(destination, "{}", output)?;
            }
        }
    }
    Ok(())
}

/// Visit each item in order in which they should appear in the children tree array.
/// The parameter of the visitor are the item, and the first_children_offset, and wether this is the flickable rectangle
#[allow(dead_code)]
pub fn build_array_helper(
    component: &Component,
    mut visit_item: impl FnMut(&ElementRc, u32, bool),
) {
    visit_item(&component.root_element, 1, false);
    visit_children(&component.root_element, 1, &mut visit_item);

    fn sub_children_count(e: &ElementRc) -> usize {
        let mut count = e.borrow().children.len();
        for i in &e.borrow().children {
            count += sub_children_count(i);
        }
        if is_flickable(e) {
            count += 1;
        }
        count
    }

    fn visit_children(
        item: &ElementRc,
        children_offset: u32,
        visit_item: &mut impl FnMut(&ElementRc, u32, bool),
    ) {
        let mut offset = children_offset + item.borrow().children.len() as u32;

        if is_flickable(item) {
            visit_item(item, offset, true);
            offset += 1;
        }

        for i in &item.borrow().children {
            visit_item(i, offset, false);
            offset += sub_children_count(i) as u32;
        }

        let mut offset = children_offset + item.borrow().children.len() as u32;

        if is_flickable(item) {
            offset += 1;
        }

        for e in &item.borrow().children {
            visit_children(e, offset, visit_item);
            offset += sub_children_count(e) as u32;
        }
    }
}

pub fn is_flickable(e: &ElementRc) -> bool {
    match &e.borrow().base_type {
        crate::typeregister::Type::Native(n) if n.class_name == "Flickable" => true,
        _ => false,
    }
}
