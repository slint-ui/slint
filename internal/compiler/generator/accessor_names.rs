// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Single source of truth for the names of the public accessors emitted by the
//! Rust and C++ backends for a property, callback, or function.
//!
//! Both backends apply only one transformation to the declaration name (`-` →
//! `_`) and then prefix it with `get_`/`set_`/`invoke_`/`on_` depending on the
//! declaration kind. Keeping that mapping here means codegen sites and any
//! consumer that needs to refer to accessors by name (notably the LSP, when
//! computing cross-language rename edits) cannot drift.

use smol_str::{SmolStr, format_smolstr};

/// Kind of a public Slint declaration whose accessors we emit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DeclarationKind {
    Property,
    Callback,
    Function,
}

/// Individual accessor a backend emits for a public declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AccessorKind {
    /// `get_<name>` — property getter.
    Getter,
    /// `set_<name>` — property setter.
    Setter,
    /// `invoke_<name>` — callback or function caller.
    Invoker,
    /// `on_<name>` — callback handler installer.
    Handler,
}

impl AccessorKind {
    pub const fn prefix(self) -> &'static str {
        match self {
            Self::Getter => "get_",
            Self::Setter => "set_",
            Self::Invoker => "invoke_",
            Self::Handler => "on_",
        }
    }
}

impl DeclarationKind {
    /// Accessor kinds emitted for this declaration, in the order both backends
    /// declare them.
    pub const fn accessor_kinds(self) -> &'static [AccessorKind] {
        match self {
            Self::Property => &[AccessorKind::Getter, AccessorKind::Setter],
            Self::Callback => &[AccessorKind::Invoker, AccessorKind::Handler],
            Self::Function => &[AccessorKind::Invoker],
        }
    }
}

/// The accessor name emitted by the Rust backend for a declaration named
/// `name` (e.g. `"get_foo_bar"` for `("foo-bar", Getter)`).
///
/// Mirrors the suffix transformation in [`super::rust::ident`]. The prefix
/// guarantees the result is never a Rust keyword, so no raw-identifier
/// escaping is applied here.
pub fn rust_accessor_name(name: &str, accessor: AccessorKind) -> SmolStr {
    format_accessor_name(name, accessor)
}

/// The accessor name emitted by the C++ backend for a declaration named
/// `name`.
///
/// Mirrors [`super::cpp::concatenate_ident`]. Today this is identical to
/// [`rust_accessor_name`]; the helpers are kept separate so the two backends
/// can diverge cleanly if either ever needs language-specific escaping.
pub fn cpp_accessor_name(name: &str, accessor: AccessorKind) -> SmolStr {
    format_accessor_name(name, accessor)
}

fn format_accessor_name(name: &str, accessor: AccessorKind) -> SmolStr {
    let prefix = accessor.prefix();
    if name.contains('-') {
        let snake = name.replace('-', "_");
        format_smolstr!("{prefix}{snake}")
    } else {
        format_smolstr!("{prefix}{name}")
    }
}

/// Same as [`rust_accessor_name`] but wrapped in a [`proc_macro2::Ident`] for
/// direct use in `quote!` templates.
#[cfg(feature = "rust")]
pub fn rust_accessor_ident(name: &str, accessor: AccessorKind) -> proc_macro2::Ident {
    quote::format_ident!("{}", rust_accessor_name(name, accessor).as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accessor_name_mapping() {
        // (input_name, accessor_kind, expected_accessor)
        // Rust and C++ produce identical output today, so each row is asserted
        // against both helpers in one loop. Adding a case anywhere -- new
        // kebab-case form, new keyword collision, new whitespace edge case --
        // is a single row here, not two parallel asserts.
        const CASES: &[(&str, AccessorKind, &str)] = &[
            // Bare snake-case / single-word inputs.
            ("foo", AccessorKind::Getter, "get_foo"),
            ("foo", AccessorKind::Setter, "set_foo"),
            ("clicked", AccessorKind::Invoker, "invoke_clicked"),
            ("clicked", AccessorKind::Handler, "on_clicked"),
            ("foo_bar", AccessorKind::Getter, "get_foo_bar"),
            // Kebab-case becomes snake-case.
            ("foo-bar", AccessorKind::Getter, "get_foo_bar"),
            ("multi-word-name", AccessorKind::Setter, "set_multi_word_name"),
            ("do-it", AccessorKind::Invoker, "invoke_do_it"),
            // The accessor prefix neutralizes language keywords on both sides,
            // so neither backend needs an escape pass.
            ("type", AccessorKind::Getter, "get_type"),
            ("if", AccessorKind::Handler, "on_if"),
            ("class", AccessorKind::Getter, "get_class"),
            ("delete", AccessorKind::Invoker, "invoke_delete"),
        ];
        for &(name, kind, expected) in CASES {
            assert_eq!(rust_accessor_name(name, kind), expected, "rust: ({name:?}, {kind:?})");
            assert_eq!(cpp_accessor_name(name, kind), expected, "cpp: ({name:?}, {kind:?})");
        }
    }

    #[test]
    fn kebab_and_snake_collapse_to_same_accessor() {
        // Both spellings produce the same accessor; the collision is intentional
        // and is the LSP scanner's concern, not this helper's.
        assert_eq!(
            rust_accessor_name("foo-bar", AccessorKind::Getter),
            rust_accessor_name("foo_bar", AccessorKind::Getter),
        );
    }

    #[test]
    fn declaration_kind_accessor_sets() {
        const CASES: &[(DeclarationKind, &[AccessorKind])] = &[
            (DeclarationKind::Property, &[AccessorKind::Getter, AccessorKind::Setter]),
            (DeclarationKind::Callback, &[AccessorKind::Invoker, AccessorKind::Handler]),
            (DeclarationKind::Function, &[AccessorKind::Invoker]),
        ];
        for &(kind, expected) in CASES {
            assert_eq!(kind.accessor_kinds(), expected, "{kind:?}");
        }
    }
}
