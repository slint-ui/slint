// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

// Copyright © SixtyFPS GmbH <info@slint.dev>

pub struct Visitor(serde_json::Value, pub bool);

impl syn::visit_mut::VisitMut for Visitor {
    fn visit_attribute_mut(&mut self, i: &mut syn::Attribute) {
        if i.meta.path().is_ident("doc") {
            if let syn::Meta::NameValue(syn::MetaNameValue {
                value: syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit), .. }),
                ..
            }) = &mut i.meta
            {
                let mut doc = lit.value();
                self.process_string(&mut doc);
                *lit = syn::LitStr::new(&doc, lit.span());
            }
        }
    }
}

impl Visitor {
    pub fn new() -> Self {
        let link_data: serde_json::Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/link-data.json"
        )))
        .expect("Failed to parse link-data.json");
        Self(link_data, false)
    }

    pub fn process_string(&mut self, doc: &mut String) {
        const NEEDLE: &str = "slint:";
        let mut begin = 0;
        // search for all occurrences of "slint:foo" and replace it with the link from link-data.json
        while let Some(pos) = doc[begin..].find(NEEDLE).map(|x| x + begin) {
            if doc[pos..].starts_with("slint::") {
                begin = pos + NEEDLE.len();
                continue;
            }
            let end = doc[pos + NEEDLE.len()..]
                .find([' ', '\n', ']', ')'])
                .expect("Failed to find end of link");
            let link = &doc[pos + NEEDLE.len()..][..end];
            let dst = if let Some(rust_link) = link.strip_prefix("rust:") {
                format!(
                    "https://releases.slint.dev/{}/docs/rust/{rust_link}",
                    env!("CARGO_PKG_VERSION"),
                )
            } else if let Some(dst) = self.0.get(link) {
                let dst = dst
                    .get("href")
                    .expect("Missing href in link-data.json")
                    .as_str()
                    .expect("invalid string in link-data.json");
                format!("https://releases.slint.dev/{}/docs/slint/{dst}", env!("CARGO_PKG_VERSION"),)
            } else {
                panic!("Unknown link {}", link);
            };
            doc.replace_range(pos..pos + NEEDLE.len() + link.len(), &dst);
            begin = pos + dst.len();
            self.1 = true;
        }
    }
}

#[test]
fn test_slint_doc() {
    let mut visitor = Visitor::new();

    let mut string = r"
    Test [SomeLink](slint:index)
    Not in a link: slint:index xxx
    slint::index is not a link
    slint:index is a link
    rust link: slint:rust:foobar
     "
    .to_owned();

    visitor.process_string(&mut string);
    assert!(visitor.1);
    assert_eq!(
        string,
        format!(
            r"
    Test [SomeLink](https://releases.slint.dev/{0}/docs/slint/)
    Not in a link: https://releases.slint.dev/{0}/docs/slint/ xxx
    slint::index is not a link
    https://releases.slint.dev/{0}/docs/slint/ is a link
    rust link: https://releases.slint.dev/{0}/docs/rust/foobar
     ",
            env!("CARGO_PKG_VERSION")
        )
    );
}
