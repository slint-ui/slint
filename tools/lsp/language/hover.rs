// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use super::token_info::TokenInfo;
use crate::common::DocumentCache;
use i_slint_compiler::langtype::{ElementType, PropertyLookupResult, Type};
use i_slint_compiler::parser::SyntaxToken;
use itertools::Itertools as _;
use lsp_types::{Hover, HoverContents, MarkupContent};

pub fn get_tooltip(document_cache: &mut DocumentCache, token: SyntaxToken) -> Option<Hover> {
    let token_info = crate::language::token_info::token_info(document_cache, token)?;
    let contents = match token_info {
        TokenInfo::Type(ty) => from_plain_text(ty.to_string()),
        TokenInfo::ElementType(e) => match e {
            ElementType::Component(c) => {
                if c.is_global() {
                    from_slint_code(&format!("global {}", c.id))
                } else {
                    from_slint_code(&format!("component {}", c.id))
                }
            }
            ElementType::Builtin(b) => from_plain_text(format!("{} (builtin)", b.name)),
            _ => return None,
        },
        TokenInfo::ElementRc(e) => {
            let e = e.borrow();
            let component = &e.enclosing_component.upgrade().unwrap();
            if component.is_global() {
                from_slint_code(&format!("global {}", component.id))
            } else if e.id.is_empty() {
                from_slint_code(&format!("{} {{ /*...*/ }}", e.base_type))
            } else {
                from_slint_code(&format!("{} := {} {{ /*...*/ }}", e.id, e.base_type))
            }
        }
        TokenInfo::NamedReference(nr) => {
            let prop_info = nr.element().borrow().lookup_property(nr.name());
            from_prop_result(prop_info)?
        }
        TokenInfo::EnumerationValue(v) => from_slint_code(&format!("{}.{}", v.enumeration.name, v)),
        TokenInfo::FileName(_) => return None,
        // Todo: this can happen when there is some syntax error
        TokenInfo::LocalProperty(_) | TokenInfo::LocalCallback(_) => return None,
        TokenInfo::IncompleteNamedReference(el, name) => {
            let prop_info = el.lookup_property(&name);
            from_prop_result(prop_info)?
        }
    };

    Some(Hover { contents: HoverContents::Markup(contents), range: None })
}

fn from_prop_result(prop_info: PropertyLookupResult) -> Option<MarkupContent> {
    let pure = if prop_info.declared_pure.is_some_and(|x| x) { "pure " } else { "" };
    if let Type::Callback(callback) = &prop_info.property_type {
        let ret = callback.return_type.as_ref().map(|x| format!(" -> {}", x)).unwrap_or_default();
        let args = callback.args.iter().map(|x| x.to_string()).join(", ");
        Some(from_slint_code(&format!("{pure}callback {}({args}){ret}", prop_info.resolved_name)))
    } else if let Type::Function(function) = &prop_info.property_type {
        let ret = if matches!(function.return_type, Type::Void) {
            String::new()
        } else {
            format!(" -> {}", function.return_type)
        };
        let args = function.args.iter().map(|x| x.to_string()).join(", ");
        Some(from_slint_code(&format!("{pure}function {}({args}){ret}", prop_info.resolved_name)))
    } else if prop_info.property_type.is_property_type() {
        Some(from_slint_code(&format!(
            "property <{}> {}",
            prop_info.property_type, prop_info.resolved_name
        )))
    } else {
        None
    }
}

fn from_plain_text(value: String) -> MarkupContent {
    MarkupContent { kind: lsp_types::MarkupKind::PlainText, value }
}

fn from_slint_code(value: &str) -> MarkupContent {
    MarkupContent {
        kind: lsp_types::MarkupKind::Markdown,
        value: format!("```slint\n{value}\n```"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use i_slint_compiler::parser::TextSize;

    #[test]
    fn test_tooltip() {
        let source = r#"
global Glob {
  in-out property <{a:int,b:float}> hello_world;
  callback cb(string, int) -> [int];
  public pure function fn_glob(abc: int) {}
}
component TA inherits TouchArea {
  in property <string> hello;
  callback xyz(string, int);
  pure callback www;
}
enum Eee { E1, E2, E3 }
export component Test {
  property <string> root-prop;
  function fn_loc() -> int { 42 }
  the-ta := TA {
      property<int> local-prop: root-prop.to-float();
      hello: Glob.hello_world.a;
      xyz(abc, def) => {
         self.www();
         self.enabled = false;
         Glob.fn_glob(local-prop);
         Glob.cb("xxx", 45);
         root.fn_loc();
      }
      property <Eee> e: Eee.E2;
  }
  Rectangle {
    background: red;
    border-color: self.background;
  }
}"#;
        let (mut dc, uri, _) = crate::language::test::loaded_document_cache(source.into());
        let doc = dc.get_document(&uri).unwrap().node.clone().unwrap();

        let find_tk = |needle: &str, offset: TextSize| {
            crate::language::token_at_offset(
                &doc,
                TextSize::new(
                    source.find(needle).unwrap_or_else(|| panic!("'{needle}' not found")) as u32,
                ) + offset,
            )
            .unwrap()
        };

        #[track_caller]
        fn assert_tooltip(h: Option<Hover>, str: &str) {
            match h.unwrap().contents {
                HoverContents::Markup(m) => assert_eq!(m.value, str),
                x => panic!("Found {x:?} ({str})"),
            }
        }

        // properties
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("hello: Glob", 0.into())),
            "```slint\nproperty <string> hello\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("Glob.hello_world", 8.into())),
            "```slint\nproperty <{ a: int,b: float,}> hello-world\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("self.enabled", 5.into())),
            "```slint\nproperty <bool> enabled\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("fn_glob(local-prop)", 10.into())),
            "```slint\nproperty <int> local-prop\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("root-prop.to-float", 1.into())),
            "```slint\nproperty <string> root-prop\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("background: red", 0.into())),
            "```slint\nproperty <brush> background\n```",
        );
        // callbacks
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("self.www", 5.into())),
            "```slint\npure callback www()\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("xyz(abc", 0.into())),
            "```slint\ncallback xyz(string, int)\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("Glob.cb(", 6.into())),
            "```slint\ncallback cb(string, int) -> [int]\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("fn_glob(local-prop)", 1.into())),
            "```slint\npure function fn-glob(int)\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("root.fn_loc", 8.into())),
            "```slint\nfunction fn-loc() -> int\n```",
        );
        // elements
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("self.enabled", 0.into())),
            "```slint\nthe-ta := TA { /*...*/ }\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("self.background", 0.into())),
            "```slint\nRectangle { /*...*/ }\n```",
        );
        // global
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("hello: Glob", 8.into())),
            "```slint\nglobal Glob\n```",
        );

        //components
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("Rectangle {", 8.into())),
            "Rectangle (builtin)",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("the-ta := TA {", 11.into())),
            "```slint\ncomponent TA\n```",
        );

        // enums
        assert_tooltip(get_tooltip(&mut dc, find_tk("Eee.E2", 0.into())), "enum Eee");
        assert_tooltip(get_tooltip(&mut dc, find_tk("Eee.E2", 5.into())), "```slint\nEee.E2\n```");
    }
}
