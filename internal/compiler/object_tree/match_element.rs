// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Types and functions for the 'match' element.

use std::collections::HashSet;

use smol_str::SmolStr;

use crate::expression_tree::{Expression, Unit};
use crate::langtype::{EnumerationValue, Type};

#[derive(Clone, PartialEq)]
pub enum CaseValue {
    Number(f64, Unit),
    String(SmolStr),
    Bool(bool),
    Enumeration(EnumerationValue),
}

impl CaseValue {
    pub fn new(value: &Expression) -> Option<Self> {
        match value {
            Expression::Cast { from, .. } => Self::new(from),
            Expression::UnaryOp { sub, op: '-' } => match Self::new(sub)? {
                Self::Number(number, unit) => Some(Self::Number(-number, unit)),
                _ => None,
            },
            Expression::NumberLiteral(number, unit) => Some(Self::Number(*number, *unit)),
            Expression::StringLiteral(string) => Some(Self::String(string.clone())),
            Expression::BoolLiteral(boolean) => Some(Self::Bool(*boolean)),
            Expression::EnumerationValue(value) => Some(Self::Enumeration(value.clone())),
            _ => None, // For invalid non-literals
        }
    }
}

// `f64` has no total order/equality (NaN), but case values are always parsed
// literals, never NaN, so treating `CaseValue` as `Eq`/`Hash` is sound here.
impl Eq for CaseValue {}

impl std::hash::Hash for CaseValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        core::mem::discriminant(self).hash(state);
        match self {
            // Normalize -0.0 to 0.0 so the hash agrees with `==`, which treats them as equal.
            CaseValue::Number(number, unit) => {
                debug_assert!(!number.is_nan());
                (if *number == 0.0 { 0.0 } else { *number }).to_bits().hash(state);
                unit.hash(state);
            }
            CaseValue::String(string) => string.hash(state),
            CaseValue::Bool(boolean) => boolean.hash(state),
            CaseValue::Enumeration(value) => value.hash(state),
        }
    }
}

impl std::fmt::Display for CaseValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaseValue::Number(number, unit) => write!(f, "{number}{unit}"),
            CaseValue::String(string) => write!(f, "{string:?}"),
            CaseValue::Bool(boolean) => write!(f, "{boolean}"),
            CaseValue::Enumeration(value) => write!(f, "{value}"),
        }
    }
}

#[test]
fn test_case_value_display() {
    assert_eq!(CaseValue::Number(1.0, Unit::Px).to_string(), "1px");
    assert_eq!(CaseValue::Number(-2.5, Unit::Ms).to_string(), "-2.5ms");
    assert_eq!(CaseValue::Number(3.0, Unit::None).to_string(), "3");
    assert_eq!(CaseValue::String("a\"b".into()).to_string(), r#""a\"b""#);
}

pub enum MatchSubjectDomain {
    Unknown,
    Exhaustive(Vec<CaseValue>),
    Unbounded,
}

impl MatchSubjectDomain {
    pub fn of(subject_type: &Type) -> Self {
        match &subject_type {
            Type::Bool => {
                MatchSubjectDomain::Exhaustive(vec![CaseValue::Bool(true), CaseValue::Bool(false)])
            }
            Type::Enumeration(enumeration) => {
                let values = (0..enumeration.values.len())
                    .map(|value| {
                        CaseValue::Enumeration(EnumerationValue {
                            value,
                            enumeration: enumeration.clone(),
                        })
                    })
                    .collect();
                MatchSubjectDomain::Exhaustive(values)
            }
            Type::Invalid => MatchSubjectDomain::Unknown,
            _ => MatchSubjectDomain::Unbounded,
        }
    }
}

pub fn missing_case_values<'a, 'b>(
    domain: &'a [CaseValue],
    covered: impl IntoIterator<Item = &'b CaseValue>,
) -> Vec<&'a CaseValue> {
    #[allow(
        clippy::mutable_key_type,
        reason = "CaseValue's Enumeration variant has interior mutability, but Eq/Hash only use its Arc pointer and index, never the Enumeration's contents"
    )]
    let covered: HashSet<&CaseValue> = covered.into_iter().collect();
    domain.iter().filter(|value| !covered.contains(value)).collect()
}
