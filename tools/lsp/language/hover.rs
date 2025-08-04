// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::common::{
    self,
    token_info::{token_info, TokenInfo},
};
use crate::util;
use i_slint_compiler::langtype::{ElementType, Type};
use i_slint_compiler::object_tree::ElementRc;
use i_slint_compiler::parser::{SyntaxKind, SyntaxNode, SyntaxToken};
use itertools::Itertools as _;
use lsp_types::{Hover, HoverContents, MarkupContent};

pub fn get_tooltip(
    document_cache: &mut common::DocumentCache,
    token: SyntaxToken,
) -> Option<Hover> {
    let token_info = token_info(document_cache, token.clone())?;
    let documentation = token_info.declaration().and_then(|x| extract_documentation(&x));
    let documentation = documentation.as_deref();
    let contents = match token_info {
        TokenInfo::Type(ty) => match ty {
            Type::Enumeration(e) => from_slint_code(&format!("enum {}", e.name), documentation),
            Type::Struct(s) if s.name.is_some() => {
                from_slint_code(&format!("struct {}", s.name.as_ref().unwrap()), documentation)
            }
            _ => from_plain_text(ty.to_string()),
        },
        TokenInfo::ElementType(e) => match e {
            ElementType::Component(c) => {
                if c.is_global() {
                    from_slint_code(&format!("global {}", c.id), documentation)
                } else {
                    from_slint_code(&format!("component {}", c.id), documentation)
                }
            }
            ElementType::Builtin(b) => from_plain_text(format!("{} (builtin)", b.name)),
            _ => return None,
        },
        TokenInfo::ElementRc(e) => {
            let e = e.borrow();
            let component = &e.enclosing_component.upgrade().unwrap();
            if component.is_global() {
                from_slint_code(&format!("global {}", component.id), documentation)
            } else if e.id.is_empty() {
                from_slint_code(&format!("{} {{ /*...*/ }}", e.base_type), documentation)
            } else {
                from_slint_code(
                    &format!("{} := {} {{ /*...*/ }}", e.id, e.base_type),
                    documentation,
                )
            }
        }
        TokenInfo::NamedReference(nr) => {
            from_property_in_element(&nr.element(), nr.name(), documentation)?
        }
        TokenInfo::EnumerationValue(v) => {
            from_slint_code(&format!("{}.{}", v.enumeration.name, v), documentation)
        }
        TokenInfo::FileName(path) => MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value: format!("`{}`", path.to_string_lossy()),
        },
        TokenInfo::Image(path) => MarkupContent {
            kind: lsp_types::MarkupKind::Markdown,
            value: format!("![{0}]({0})", path.to_string_lossy()),
        },
        // Todo: this can happen when there is some syntax error
        TokenInfo::LocalProperty(_) | TokenInfo::LocalCallback(_) | TokenInfo::LocalFunction(_) => {
            return None
        }
        TokenInfo::IncompleteNamedReference(el, name) => {
            from_property_in_type(&el, &name, documentation)?
        }
    };

    Some(Hover {
        contents: HoverContents::Markup(contents),
        range: Some(util::token_to_lsp_range(&token)),
    })
}

// Given a token that declares something, find a comment before that token that could be a documentation for this
fn extract_documentation(declaration: &SyntaxNode) -> Option<String> {
    let mut token = declaration.first_token()?;
    // Loop back to find the the previous line \n
    loop {
        if token.kind() == SyntaxKind::Whitespace {
            let mut ln = token.text().bytes().filter(|c| *c == b'\n');
            // One \n
            if ln.next().is_some() {
                // Two \n
                if ln.next().is_some() {
                    return None;
                }
                token = token.prev_token()?;
                break;
            }
        }
        token = token.prev_token()?;
    }

    // find the comment
    let mut result = String::new();
    while token.kind() == SyntaxKind::Comment {
        let text = token.text().to_string();
        token = if let Some(token) = token.prev_token() { token } else { break };
        if token.kind() == SyntaxKind::Whitespace {
            let mut ln = token.text().bytes().filter(|c| *c == b'\n');
            // One \n
            if ln.next().is_some() {
                result = format!("{}{text}{result}", token.text());
                // Two \n
                if ln.next().is_some() {
                    break;
                }
                token = if let Some(token) = token.prev_token() { token } else { break };
                continue;
            }
        }
        break;
    }

    if result.is_empty() {
        return None;
    }

    // De-ident the comment
    let indentation_size =
        result.lines().filter_map(|x| x.find(|x| x != ' ' && x != '\t')).min()?;
    let mut result2 = String::new();
    for line in result.lines().skip_while(|p| p.trim().is_empty()) {
        if line.len() > indentation_size {
            result2.push_str(&line[indentation_size..].trim_end());
        }
        result2.push('\n');
    }
    if result2.ends_with("\n\n") {
        result2.pop(); // remove the last newline
    }
    Some(result2)
}

fn from_property_in_element(
    element: &ElementRc,
    name: &str,
    documentation: Option<&str>,
) -> Option<MarkupContent> {
    if let Some(decl) = element.borrow().property_declarations.get(name) {
        return property_tooltip(
            &decl.property_type,
            name,
            decl.pure.unwrap_or(false),
            documentation,
        );
    }
    from_property_in_type(&element.borrow().base_type, name, documentation)
}

fn from_property_in_type(
    base: &ElementType,
    name: &str,
    documentation: Option<&str>,
) -> Option<MarkupContent> {
    match base {
        ElementType::Component(c) => from_property_in_element(&c.root_element, name, documentation),
        ElementType::Builtin(b) => {
            let resolved_name = b.native_class.lookup_alias(name).unwrap_or(name);
            let info = b.properties.get(resolved_name)?;
            property_tooltip(&info.ty, name, false, documentation)
        }
        _ => None,
    }
}

fn property_tooltip(
    ty: &Type,
    name: &str,
    pure: bool,
    documentation: Option<&str>,
) -> Option<MarkupContent> {
    let pure = if pure { "pure " } else { "" };
    if let Type::Callback(callback) = ty {
        let sig = signature_from_function_ty(callback);
        Some(from_slint_code(&format!("{pure}callback {name}{sig}"), documentation))
    } else if let Type::Function(function) = &ty {
        let sig = signature_from_function_ty(function);
        Some(from_slint_code(&format!("{pure}function {name}{sig}"), documentation))
    } else if ty.is_property_type() {
        Some(from_slint_code(&format!("property <{ty}> {name}"), documentation))
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

fn from_slint_code(value: &str, documentation: Option<&str>) -> MarkupContent {
    let documentation = documentation.unwrap_or("");
    MarkupContent {
        kind: lsp_types::MarkupKind::Markdown,
        value: format!("```slint\n{documentation}{value}\n```"),
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

/// Docs for
/// the Glob global
global Glob {
  in-out property <{a:int,b:float}> hello_world;
  // not docs

  callback cb(string, int) -> [int];
  /** The fn_glob function */
  public pure
  function fn_glob(abc: int) {}
}

/// TA is a component
component TA inherits TouchArea { // not docs
  in property <string> hello; // not docs
  /** Docs for
   * the xyz callback
   */
  callback xyz(string, int);
  /*not docs */ pure callback www;
}
/// Here some docs for Eee
enum Eee { E1, E2, E3 }
export component Test { // not docs
  // root-prop is a property
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
        let documentation = dc.get_document(&uri).unwrap().node.clone().unwrap();

        let find_tk = |needle: &str, offset: TextSize| {
            crate::language::token_at_offset(
                &documentation,
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
            "```slint\n// root-prop is a property\nproperty <string> root-prop\n```",
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
            "```slint\n/** Docs for\n * the xyz callback\n */\ncallback xyz(string, int)\n```",
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
            "```slint\n/** The fn_glob function */\npure function fn-glob(abc: int)\n```",
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
            "```slint\n/// Docs for\n/// the Glob global\nglobal Glob\n```",
        );

        //components
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("Rectangle {", 8.into())),
            "Rectangle (builtin)",
        );
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("the-ta := TA {", 11.into())),
            "```slint\n/// TA is a component\ncomponent TA\n```",
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
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("Eee.E2", 0.into())),
            "```slint\n/// Here some docs for Eee\nenum Eee\n```",
        );
        // FIXME: We get the comments for the enum instead of the value
        assert_tooltip(
            get_tooltip(&mut dc, find_tk("Eee.E2", 5.into())),
            "```slint\n/// Here some docs for Eee\nEee.E2\n```",
        );
    }
}
