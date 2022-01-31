// Copyright © SixtyFPS GmbH <info@sixtyfps.io>
// SPDX-License-Identifier: MIT OR Apache-2.0

/*!
Document your crate's feature flags.

This crate provide a macro that extracts "documentation" comments from Cargo.toml

In order to use this crate, simply  add `#![doc = document_features::document_features!()]`
within your crate documentation.
The [`document_features!()`] reads the Cargo.toml file and generate a markdown string
suitable to be used within the documentation

Basic example:

```rust
//! Normal crate documentation goes here,
//!
//! ## Feature flags
#![doc = document_features::document_features!()]

// rest of the crate goes here.
```

## Documentation format:

The documentation of the features itself goes in the `Cargo.toml`.

Only the portion in the `[features]` section will be analyzed.
Just like rust has documentation comments like `///` and `//!`, the macro will
understands the comments that start with `## ` and `#! `. Note that the space
is important. Lines starting with `###` will not be understood as doc comment.

`## ` comments are meant to be *over* a feature and document that feature.
there can be several `## ` comments, but they must always be followed by a
feature name, and no other `#! ` comments in between.

`#! ` comments are not associated with a particular feature, and will be printed
in where they occurs. They are useful to do some grouping for example

## Examples:

*/
// Note: because rustdoc escapes the first `#` of a line starting with `#`,
// these docs comments have one more `#` ,
#![doc = self_test!(/**
[package]
name = "..."
## ...

[features]
default = ["foo"]
##! This comments goes on top

### The foo feature is enabling the `foo` functions
foo = []

### The bar feature enable the bar module
bar = []

##! ### Experimental features
##! The following features are experimental

### Enable the fusion reactor
fusion = []
*/
=>
    /**
This comments goes on top
* **`foo`** *(enabled by default)* —  The foo feature is enabling the `foo` functions
* **`bar`** —  The bar feature enable the bar module
#### Experimental features
The following features are experimental
* **`fusion`** —  Enable the fusion reactor
*/
)]

extern crate proc_macro;
use proc_macro::TokenStream;

use std::collections::HashSet;
use std::fmt::Write;
use std::path::Path;
use std::str::FromStr;

fn error(e: &str) -> TokenStream {
    TokenStream::from_str(&format!("::core::compile_error!{{\"{}\"}}", e.escape_default())).unwrap()
}

#[proc_macro]
pub fn document_features(_: TokenStream) -> TokenStream {
    document_features_impl().unwrap_or_else(std::convert::identity)
}

fn document_features_impl() -> Result<TokenStream, TokenStream> {
    let path = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let mut cargo_toml = std::fs::read_to_string(Path::new(&path).join("Cargo.toml"))
        .map_err(|e| error(&format!("Can't open Cargo.toml: {:?}", e)))?;

    if !cargo_toml.contains("\n##") && !cargo_toml.contains("\n#!") {
        // On crates.io, Cargo.toml is usually "normalized" and stripped of all comments.
        // The original Cargo.toml has been renamed Cargo.toml.orig
        if let Ok(orig) = std::fs::read_to_string(Path::new(&path).join("Cargo.toml.orig")) {
            if orig.contains("##") || orig.contains("#!") {
                cargo_toml = orig;
            }
        }
    }

    let result = process_toml(&cargo_toml).map_err(|e| error(&e))?;
    Ok(std::iter::once(proc_macro::TokenTree::from(proc_macro::Literal::string(&result))).collect())
}

fn process_toml(cargo_toml: &str) -> Result<String, String> {
    // Get all lines between the "[features]" and the next block
    let lines = cargo_toml
        .lines()
        .map(str::trim)
        .skip_while(|l| !l.starts_with("[features]"))
        .skip(1) // skip the [features] line
        .take_while(|l| !l.starts_with("["))
        // and skip empty lines and comments that are not docs comments
        .filter(|l| {
            !l.is_empty() && (!l.starts_with("#") || l.starts_with("##") || l.starts_with("#!"))
        });
    let mut top_comment = String::new();
    let mut current_comment = String::new();
    let mut features = vec![];
    let mut default_features = HashSet::new();
    for line in lines {
        if let Some(x) = line.strip_prefix("#!") {
            if !x.is_empty() && !x.starts_with(" ") {
                continue; // it's not a doc comment
            }
            if !current_comment.is_empty() {
                return Err("Cannot mix ## and #! comments between features.".into());
            }
            writeln!(top_comment, "{}", x).unwrap();
        } else if let Some(x) = line.strip_prefix("##") {
            if !x.is_empty() && !x.starts_with(" ") {
                continue; // it's not a doc comment
            }
            writeln!(current_comment, "{}", x).unwrap();
        } else if let Some((dep, rest)) = line.split_once("=") {
            if dep.trim() == "default" {
                let defaults = rest
                    .trim()
                    .strip_prefix("[")
                    .and_then(|r| r.strip_suffix("]"))
                    .ok_or_else(|| format!("Parse error while parsing dependency {}", dep))?
                    .split(",")
                    .map(|d| d.trim().trim_matches(|c| c == '"' || c == '\'').trim())
                    .filter(|d| !d.is_empty());
                default_features.extend(defaults);
            } else {
                // Do not show features that do not have documentation
                if !current_comment.is_empty() {
                    features.push((
                        dep.trim(),
                        std::mem::take(&mut top_comment),
                        std::mem::take(&mut current_comment),
                    ));
                }
            }
        } else {
            return Err(format!("Parse error while processing the line:\n{}", line));
        }
    }
    if !current_comment.is_empty() {
        return Err("Found comment not associated with a feature".into());
    }
    if features.is_empty() {
        return Err("Could not find features in Cargo.toml".into());
    }
    let mut result = String::new();
    for (f, top, comment) in features {
        let default = if default_features.contains(f) { " *(enabled by default)*" } else { "" };
        if !comment.trim().is_empty() {
            write!(result, "{}* **`{}`**{} — {}", top, f, default, comment).unwrap();
        } else {
            writeln!(result, "{}* **`{}`**{}", top, f, default).unwrap();
        }
    }
    result += &top_comment;
    Ok(result)
}

#[cfg(feature = "self-test")]
#[proc_macro]
#[doc(hidden)]
/// Helper macro for the tests. Do not use
pub fn self_test_helper(input: TokenStream) -> TokenStream {
    process_toml((&input).to_string().trim_matches(|c| c == '"' || c == '#')).map_or_else(
        |e| error(&e),
        |r| std::iter::once(proc_macro::TokenTree::from(proc_macro::Literal::string(&r))).collect(),
    )
}

#[cfg(feature = "self-test")]
macro_rules! self_test {
    (#[doc = $toml:literal] => #[doc = $md:literal]) => {
        concat!(
            "\n`````rust\n\
            fn normalize_md(md : &str) -> String {
               md.lines().skip_while(|l| l.is_empty()).map(|l| l.trim())
                .collect::<Vec<_>>().join(\"\\n\")
            }
            assert_eq!(normalize_md(document_features::self_test_helper!(",
            stringify!($toml),
            ")), normalize_md(",
            stringify!($md),
            "));\n`````\n\n"
        )
    };
}

#[cfg(not(feature = "self-test"))]
macro_rules! self_test {
    (#[doc = $toml:literal] => #[doc = $md:literal]) => {
        concat!(
            "This contents in Cargo.toml:\n`````toml",
            $toml,
            "\n`````\n Generates the following:\n\
            <table><tr><th>Preview</th></tr><tr><td>\n\n",
            $md,
            "\n</td></tr></table\n",
        )
    };
}

#[cfg(test)]
mod tests {
    use super::process_toml;

    #[track_caller]
    fn test_error(toml: &str, expected: &str) {
        let err = process_toml(toml).unwrap_err();
        assert!(err.contains(expected), "{:?} does not contain {:?}", err, expected)
    }

    #[test]
    fn parse_errors() {
        test_error(
            r#"
[features]
[dependencies]
foo = 4;
"#,
            "Could not find features",
        );

        test_error(
            r#"
[packages]
[dependencies]
"#,
            "Could not find features",
        );

        test_error(
            r#"
[features]
ff = []
abcd
efgh
[dependencies]
"#,
            "Parse error while processing the line:\nabcd",
        );

        test_error(
            r#"
[features]
## dd
## ff
#! ee
## ff
"#,
            "Cannot mix",
        );

        test_error(
            r#"
[features]
## dd
"#,
            "not associated with a feature",
        );

        test_error(
            r#"
[features]
# ff
foo = []
default = [
#ffff
# ff
"#,
            "Parse error while parsing dependency default",
        );
    }

    #[test]
    fn basic() {
        assert_eq!(
            process_toml(
                r#"
[abcd]
## aaa
#! aaa
[features]#xyz
#! abc
#
###
#! def
#!
## 123
## 456
feat1 = ["plop"]
#! ghi
no_doc = []
##
feat2 = ["momo"]
#! klm
default = ["feat1", "something_else"]
#! end
        "#
            )
            .unwrap(),
            " abc\n def\n\n* **`feat1`** *(enabled by default)* —  123\n 456\n ghi\n* **`feat2`**\n klm\n end\n"
        );
    }
}
