// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::{
    self,
    token_info::{token_info, TokenInfo},
};
use crate::util;
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::SyntaxToken;
use itertools::Itertools as _;
use lsp_types::{Hover, HoverContents, MarkupContent};

pub fn get_tooltip(
    document_cache: &mut common::DocumentCache,
    token: SyntaxToken,
) -> Option<Hover> {
    let token_info = token_info(document_cache, token.clone())?;
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
        TokenInfo::NamedReference(nr) => from_property_in_element(&nr.element(), nr.name())?,
        TokenInfo::EnumerationValue(v) => from_slint_code(&format!("{}.{}", v.enumeration.name, v)),
        TokenInfo::FileName(path) => MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value: format!("`{}`", path.to_string_lossy()),
        },
        TokenInfo::Image(path) => MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value: format!("![{0}]({0})", path.to_string_lossy()),
        },
        // Todo: this can happen when there is some syntax error
        TokenInfo::LocalProperty(_) | TokenInfo::LocalCallback(_) => return None,
        TokenInfo::IncompleteNamedReference(el, name) => from_property_in_type(&el, &name)?,
    };

    Some(Hover {
        contents: HoverContents::Markup(contents),
        range: Some(util::token_to_lsp_range(&token)),
    })
}

fn from_property_in_element(element: &ElementRc, name: &str) -> Option<MarkupContent> {
    if let Some(decl) = element.borrow().property_declarations.get(name) {
        return property_tooltip(&decl.property_type, name, decl.pure.unwrap_or(false));
    }
    from_property_in_type(&element.borrow().base_type, name)
}

fn from_property_in_type(base: &ElementType, name: &str) -> Option<MarkupContent> {
    match base {
        ElementType::Component(c) => from_property_in_element(&c.root_element, name),
        ElementType::Builtin(b) => {
            let resolved_name = b.native_class.lookup_alias(name).unwrap_or(name);
            let info = b.properties.get(resolved_name)?;
            property_tooltip(&info.ty, name, false)
        }
        _ => None,
    }
}

fn property_tooltip(ty: &Type, name: &str, pure: bool) -> Option<MarkupContent> {
    let pure = if pure { "pure " } else { "" };
    if let Type::Callback(callback) = ty {
        let sig = signature_from_function_ty(callback);
        Some(from_slint_code(&format!("{pure}callback {name}{sig}")))
    } else if let Type::Function(function) = &ty {
        let sig = signature_from_function_ty(function);
        Some(from_slint_code(&format!("{pure}function {name}{sig}")))
    } else if ty.is_property_type() {
        Some(from_slint_code(&format!("property <{ty}> {name}")))
    } else {
        None
    }
}

fn signature_from_function_ty(f: &i_slint_compiler::langtype::Function) -> String {
    let ret = if matches!(f.return_type, Type::Void) {
        String::new()
    } else {
        format!(" -> {}", f.return_type)
    };
    let args = f
        .args
        .iter()
        .zip(f.arg_names.iter().chain(std::iter::repeat(&Default::default())))
        .filter(|(x, _)| *x != &Type::ElementReference)
        .map(|(ty, name)| if !name.is_empty() { format!("{name}: {ty}") } else { ty.to_string() })
        .join(", ");
    format!("({args}){ret}")
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
import { StandardTableView } from "std-widgets.slint";
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
      pointer-event(aaa) => {}
  }
  Rectangle {
    background: red;
    border-color: self.background;
  }
  StandardTableView {
    row-pointer-event => { }
  }
  Image {
      source: @image-url("assets/unix-test.png");
  }
  Image {
      source: @image-url("assets\\windows-test.png");
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
            get_tooltip(&mut dc, find_tk("row-pointer-event", 0.into())),
            // Fixme: this uses LogicalPoint instead of Point because of implementation details
            "```slint\ncallback row-pointer-event(row: int, event: PointerEvent, position: LogicalPosition)\n```",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("pointer-event", 5.into())),
            "```slint\ncallback pointer-event(event: PointerEvent)\n```",
        );
        // functions
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("fn_glob(local-prop)", 1.into())),
            "```slint\npure function fn-glob(abc: int)\n```",
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

        // @image-url
        let target_path = uri
            .join("assets/unix-test.png")
            .unwrap()
            .to_file_path()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("\"assets/unix-test.png\"", 15.into())),
            &format!("![{target_path}]({target_path})"),
        );

        // @image-url
        let target_path = uri
            .join("assets/windows-test.png")
            .unwrap()
            .to_file_path()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("\"assets\\\\windows-test.png\"", 15.into())),
            &format!("![{target_path}]({target_path})"),
        );

        // enums
        assert_tooltip(get_tooltip(&mut dc, find_tk("Eee.E2", 0.into())), "enum Eee");
        assert_tooltip(get_tooltip(&mut dc, find_tk("Eee.E2", 5.into())), "```slint\nEee.E2\n```");
    }
}
