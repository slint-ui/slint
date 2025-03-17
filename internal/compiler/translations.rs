// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::llr::Expression;
use core::ops::Not;
use smol_str::{SmolStr, ToSmolStr};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;

#[derive(Clone, Debug)]
pub struct Translations {
    /// An array with all the array of string
    /// The first vector index is stored in the LLR.
    /// The inner vector index is the language id. (The first is the original)
    /// Only contains the string that are not having plural forms
    pub strings: Vec<Vec<Option<SmolStr>>>,
    /// An array with all the strings that are used in a plural form.
    /// The first vector index is stored in the LLR.
    /// The inner vector index is the language. (The first is the original string)
    /// The last vector contains each form
    pub plurals: Vec<Vec<Option<Vec<SmolStr>>>>,

    /// Expression is a function that maps its first and only argument (an integer)
    /// to the plural form index (an integer)
    /// It can only do basic mathematical operations.
    /// The expression cannot reference properties or variable.
    /// Only builtin math functions, and its first argument
    pub plural_rules: Vec<Option<Expression>>,

    /// The "names" of the languages
    pub languages: Vec<SmolStr>,
}

#[derive(Clone)]
pub struct TranslationsBuilder {
    result: Translations,
    /// Maps (msgid, msgid_plural, msgctx) to the index in the result
    /// (the index is in strings or plurals depending if there is a plural)
    map: HashMap<(SmolStr, SmolStr, SmolStr), usize>,

    /// The catalog containing the translations
    catalogs: Rc<Vec<polib::catalog::Catalog>>,
}

impl TranslationsBuilder {
    pub fn load_translations(path: &Path, domain: &str) -> std::io::Result<Self> {
        let mut languages = vec!["".into()];
        let mut catalogs = Vec::new();
        let mut plural_rules =
            vec![Some(plural_rule_parser::parse_rule_expression("n!=1").unwrap())];
        for l in std::fs::read_dir(path)
            .map_err(|e| std::io::Error::other(format!("Error reading directory {path:?}: {e}")))?
        {
            let l = l?;
            let path = l.path().join("LC_MESSAGES").join(format!("{domain}.po"));
            if path.exists() {
                let catalog = polib::po_file::parse(&path).map_err(|e| {
                    std::io::Error::other(format!("Error parsing {}: {e}", path.display()))
                })?;
                languages.push(l.file_name().to_string_lossy().into());
                plural_rules.push(Some(
                    plural_rule_parser::parse_rule_expression(&catalog.metadata.plural_rules.expr)
                        .map_err(|_| {
                            std::io::Error::other(format!(
                                "Error parsing plural rules in {}",
                                path.display()
                            ))
                        })?,
                ));
                catalogs.push(catalog);
            }
        }
        if catalogs.is_empty() {
            return Err(std::io::Error::other(format!(
                "No translations found. We look for files in '{}/<lang>/LC_MESSAGES/{domain}.po",
                path.display()
            )));
        }
        Ok(Self {
            result: Translations {
                strings: Vec::new(),
                plurals: Vec::new(),
                plural_rules,
                languages,
            },
            map: HashMap::new(),
            catalogs: Rc::new(catalogs),
        })
    }

    pub fn lower_translate_call(&mut self, args: Vec<Expression>) -> Expression {
        let [original, contextid, _domain, format_args, n, plural] = args
            .try_into()
            .expect("The resolving pass should have ensured that the arguments are correct");
        let original = get_string(original).expect("original must be a string");
        let contextid = get_string(contextid).expect("contextid must be a string");
        let plural = get_string(plural).expect("plural must be a string");

        let is_plural =
            !plural.is_empty() || !matches!(n, Expression::NumberLiteral(f) if f == 1.0);

        match self.map.entry((original.clone(), plural.clone(), contextid.clone())) {
            Entry::Occupied(entry) => Expression::TranslationReference {
                format_args: format_args.into(),
                string_index: *entry.get(),
                plural: is_plural.then(|| n.into()),
            },
            Entry::Vacant(entry) => {
                let messages = self.catalogs.iter().map(|catalog| {
                    catalog.find_message(
                        contextid.is_empty().not().then_some(contextid.as_str()),
                        &original,
                        is_plural.then_some(plural.as_str()),
                    )
                });
                let idx = if is_plural {
                    let messages = std::iter::once(Some(vec![original.clone(), plural.clone()]))
                        .chain(messages.map(|x| {
                            x.and_then(|x| {
                                Some(
                                    x.msgstr_plural()
                                        .ok()?
                                        .iter()
                                        .map(|x| x.to_smolstr())
                                        .collect(),
                                )
                            })
                        }))
                        .collect();
                    self.result.plurals.push(messages);
                    self.result.plurals.len() - 1
                } else {
                    let messages = std::iter::once(Some(original.clone()))
                        .chain(
                            messages
                                .map(|x| x.and_then(|x| x.msgstr().ok()).map(|x| x.to_smolstr())),
                        )
                        .collect::<Vec<_>>();
                    self.result.strings.push(messages);
                    self.result.strings.len() - 1
                };
                Expression::TranslationReference {
                    format_args: format_args.into(),
                    string_index: *entry.insert(idx),
                    plural: is_plural.then(|| n.into()),
                }
            }
        }
    }

    pub fn result(self) -> Translations {
        self.result
    }

    pub fn collect_characters_seen(&self, characters_seen: &mut impl Extend<char>) {
        characters_seen.extend(
            self.catalogs
                .iter()
                .flat_map(|catalog| {
                    catalog.messages().flat_map(|msg| {
                        msg.msgstr().ok().into_iter().chain(
                            msg.msgstr_plural()
                                .ok()
                                .into_iter()
                                .flat_map(|vec| vec.iter().map(|s| s.as_ref())),
                        )
                    })
                })
                .flat_map(|str| str.chars()),
        );
    }
}

fn get_string(plural: Expression) -> Option<SmolStr> {
    match plural {
        Expression::StringLiteral(s) => Some(s),
        _ => None,
    }
}

mod plural_rule_parser {
    use super::Expression;
    pub struct ParseError<'a>(&'static str, &'a [u8]);
    impl std::fmt::Debug for ParseError<'_> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "ParseError({}, rest={:?})", self.0, std::str::from_utf8(self.1).unwrap())
        }
    }
    pub fn parse_rule_expression(string: &str) -> Result<Expression, ParseError> {
        let ascii = string.as_bytes();
        let s = parse_expression(ascii)?;
        if !s.rest.is_empty() {
            return Err(ParseError("extra character in string", s.rest));
        }
        match s.ty {
            Ty::Number => Ok(s.expr),
            Ty::Boolean => Ok(Expression::Condition {
                condition: s.expr.into(),
                true_expr: Expression::NumberLiteral(1.).into(),
                false_expr: Expression::NumberLiteral(0.).into(),
            }),
        }
    }

    #[derive(Copy, Clone, Debug, PartialEq, Eq)]
    enum Ty {
        Number,
        Boolean,
    }

    struct ParsingState<'a> {
        expr: Expression,
        rest: &'a [u8],
        ty: Ty,
    }

    impl ParsingState<'_> {
        fn skip_whitespace(self) -> Self {
            let rest = skip_whitespace(self.rest);
            Self { rest, ..self }
        }
    }

    /// `<condition> ('?' <expr> : <expr> )?`
    fn parse_expression(string: &[u8]) -> Result<ParsingState, ParseError> {
        let string = skip_whitespace(string);
        let state = parse_condition(string)?.skip_whitespace();
        if state.ty != Ty::Boolean {
            return Ok(state);
        }
        if let Some(rest) = state.rest.strip_prefix(b"?") {
            let s1 = parse_expression(rest)?.skip_whitespace();
            let rest = s1.rest.strip_prefix(b":").ok_or(ParseError("expected ':'", s1.rest))?;
            let s2 = parse_expression(rest)?;
            if s1.ty != s2.ty {
                return Err(ParseError("incompatible types in ternary operator", s2.rest));
            }
            Ok(ParsingState {
                expr: Expression::Condition {
                    condition: state.expr.into(),
                    true_expr: s1.expr.into(),
                    false_expr: s2.expr.into(),
                },
                rest: skip_whitespace(s2.rest),
                ty: s2.ty,
            })
        } else {
            Ok(state)
        }
    }

    /// `<and_expr> ("||" <condition>)?`
    fn parse_condition(string: &[u8]) -> Result<ParsingState, ParseError> {
        let string = skip_whitespace(string);
        let state = parse_and_expr(string)?.skip_whitespace();
        if state.rest.is_empty() {
            return Ok(state);
        }
        if let Some(rest) = state.rest.strip_prefix(b"||") {
            let state2 = parse_condition(rest)?;
            if state.ty != Ty::Boolean || state2.ty != Ty::Boolean {
                return Err(ParseError("incompatible types in || operator", state2.rest));
            }
            Ok(ParsingState {
                expr: Expression::BinaryExpression {
                    lhs: state.expr.into(),
                    rhs: state2.expr.into(),
                    op: '|',
                },
                ty: Ty::Boolean,
                rest: skip_whitespace(state2.rest),
            })
        } else {
            Ok(state)
        }
    }

    /// `<cmp_expr> ("&&" <and_expr>)?`
    fn parse_and_expr(string: &[u8]) -> Result<ParsingState, ParseError> {
        let string = skip_whitespace(string);
        let state = parse_cmp_expr(string)?.skip_whitespace();
        if state.rest.is_empty() {
            return Ok(state);
        }
        if let Some(rest) = state.rest.strip_prefix(b"&&") {
            let state2 = parse_and_expr(rest)?;
            if state.ty != Ty::Boolean || state2.ty != Ty::Boolean {
                return Err(ParseError("incompatible types in || operator", state2.rest));
            }
            Ok(ParsingState {
                expr: Expression::BinaryExpression {
                    lhs: state.expr.into(),
                    rhs: state2.expr.into(),
                    op: '&',
                },
                ty: Ty::Boolean,
                rest: skip_whitespace(state2.rest),
            })
        } else {
            Ok(state)
        }
    }

    /// `<value> ('=='|'!='|'<'|'>'|'<='|'>=' <cmp_expr>)?`
    fn parse_cmp_expr(string: &[u8]) -> Result<ParsingState, ParseError> {
        let string = skip_whitespace(string);
        let mut state = parse_value(string)?;
        state.rest = skip_whitespace(state.rest);
        if state.rest.is_empty() {
            return Ok(state);
        }
        for (token, op) in [
            (b"==" as &[u8], '='),
            (b"!=", '!'),
            (b"<=", '≤'),
            (b">=", '≥'),
            (b"<", '<'),
            (b">", '>'),
        ] {
            if let Some(rest) = state.rest.strip_prefix(token) {
                let state2 = parse_cmp_expr(rest)?;
                if state.ty != Ty::Number || state2.ty != Ty::Number {
                    return Err(ParseError("incompatible types in comparison", state2.rest));
                }
                return Ok(ParsingState {
                    expr: Expression::BinaryExpression {
                        lhs: state.expr.into(),
                        rhs: state2.expr.into(),
                        op,
                    },
                    ty: Ty::Boolean,
                    rest: skip_whitespace(state2.rest),
                });
            }
        }
        Ok(state)
    }

    /// `<term> ('%' <term>)?`
    fn parse_value(string: &[u8]) -> Result<ParsingState, ParseError> {
        let string = skip_whitespace(string);
        let mut state = parse_term(string)?;
        state.rest = skip_whitespace(state.rest);
        if state.rest.is_empty() {
            return Ok(state);
        }
        if let Some(rest) = state.rest.strip_prefix(b"%") {
            let state2 = parse_term(rest)?;
            if state.ty != Ty::Number || state2.ty != Ty::Number {
                return Err(ParseError("incompatible types in % operator", state2.rest));
            }
            Ok(ParsingState {
                expr: Expression::BuiltinFunctionCall {
                    function: crate::expression_tree::BuiltinFunction::Mod,
                    arguments: vec![state.expr.into(), state2.expr.into()],
                },
                ty: Ty::Number,
                rest: skip_whitespace(state2.rest),
            })
        } else {
            Ok(state)
        }
    }

    fn parse_term(string: &[u8]) -> Result<ParsingState, ParseError> {
        let string = skip_whitespace(string);
        let state = match string.first().ok_or(ParseError("unexpected end of string", string))? {
            b'n' => ParsingState {
                expr: Expression::FunctionParameterReference { index: 0 },
                rest: &string[1..],
                ty: Ty::Number,
            },
            b'(' => {
                let mut s = parse_expression(&string[1..])?;
                s.rest = s.rest.strip_prefix(b")").ok_or(ParseError("expected ')'", s.rest))?;
                s
            }
            x if x.is_ascii_digit() => {
                let (n, rest) = parse_number(string)?;
                ParsingState { expr: Expression::NumberLiteral(n as _), rest, ty: Ty::Number }
            }
            _ => return Err(ParseError("unexpected token", string)),
        };
        Ok(state)
    }
    fn parse_number(string: &[u8]) -> Result<(i32, &[u8]), ParseError> {
        let end = string.iter().position(|&c| !c.is_ascii_digit()).unwrap_or(string.len());
        let n = std::str::from_utf8(&string[..end])
            .expect("string is valid utf-8")
            .parse()
            .map_err(|_| ParseError("can't parse number", string))?;
        Ok((n, &string[end..]))
    }
    fn skip_whitespace(mut string: &[u8]) -> &[u8] {
        // slice::trim_ascii_start when MSRV >= 1.80
        while !string.is_empty() && string[0].is_ascii_whitespace() {
            string = &string[1..];
        }
        string
    }

    #[test]
    fn test_parse_rule_expression() {
        #[track_caller]
        fn p(string: &str) -> String {
            let ctx = crate::llr::EvaluationContext {
                compilation_unit: &crate::llr::CompilationUnit {
                    public_components: Default::default(),
                    sub_components: Default::default(),
                    used_sub_components: Default::default(),
                    globals: Default::default(),
                    has_debug_info: false,
                    translations: None,
                    popup_menu: None,
                },
                current_sub_component: None,
                current_global: None,
                generator_state: (),
                parent: None,
                argument_types: &[crate::langtype::Type::Int32],
            };
            crate::llr::pretty_print::DisplayExpression(
                &parse_rule_expression(string).expect("parse error"),
                &ctx,
            )
            .to_string()
        }

        // en
        assert_eq!(p("n != 1"), "((arg_0 ! 1.0) ? 1.0 : 0.0)");
        // fr
        assert_eq!(p("n > 1"), "((arg_0 > 1.0) ? 1.0 : 0.0)");
        // ar
        assert_eq!(
            p("(n==0 ? 0 : n==1 ? 1 : n==2 ? 2 : n%100>=3 && n%100<=10 ? 3 : n%100>=11 ? 4 : 5)"),
            "((arg_0 = 0.0) ? 0.0 : ((arg_0 = 1.0) ? 1.0 : ((arg_0 = 2.0) ? 2.0 : (((Mod(arg_0, 100.0) ≥ 3.0) & (Mod(arg_0, 100.0) ≤ 10.0)) ? 3.0 : ((Mod(arg_0, 100.0) ≥ 11.0) ? 4.0 : 5.0)))))"
        );
        // ga
        assert_eq!(p("n==1 ? 0 : n==2 ? 1 : (n>2 && n<7) ? 2 :(n>6 && n<11) ? 3 : 4"), "((arg_0 = 1.0) ? 0.0 : ((arg_0 = 2.0) ? 1.0 : (((arg_0 > 2.0) & (arg_0 < 7.0)) ? 2.0 : (((arg_0 > 6.0) & (arg_0 < 11.0)) ? 3.0 : 4.0))))");
        // ja
        assert_eq!(p("0"), "0.0");
        // pl
        assert_eq!(
            p("(n==1 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)"),
            "((arg_0 = 1.0) ? 0.0 : (((Mod(arg_0, 10.0) ≥ 2.0) & ((Mod(arg_0, 10.0) ≤ 4.0) & ((Mod(arg_0, 100.0) < 10.0) | (Mod(arg_0, 100.0) ≥ 20.0)))) ? 1.0 : 2.0))",
        );

        // ru
        assert_eq!(
            p("(n%10==1 && n%100!=11 ? 0 : n%10>=2 && n%10<=4 && (n%100<10 || n%100>=20) ? 1 : 2)"),
            "(((Mod(arg_0, 10.0) = 1.0) & (Mod(arg_0, 100.0) ! 11.0)) ? 0.0 : (((Mod(arg_0, 10.0) ≥ 2.0) & ((Mod(arg_0, 10.0) ≤ 4.0) & ((Mod(arg_0, 100.0) < 10.0) | (Mod(arg_0, 100.0) ≥ 20.0)))) ? 1.0 : 2.0))",
        );
    }
}
