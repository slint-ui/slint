// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! The experimental language features, which are off unless enabled by name.

use itertools::Itertools;
use strum::IntoEnumIterator;

/// A language feature that may change or be removed without notice.
///
/// Enable it with its kebab-case name, such as `named-slots`,
/// in `SLINT_ENABLE_EXPERIMENTAL_FEATURES` or the project file's `experimental-features`.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    strum::EnumString,
    strum::Display,
    strum::EnumIter,
    serde::Deserialize,
)]
#[strum(serialize_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum ExperimentalFeature {
    /// `interface` and `implement`, and `std-widget-interfaces.slint`
    Interfaces,
    /// `@shadowable`
    Shadowable,
    /// Closures, and the array functions that take them, such as `any` and `index-of`
    Closures,
    /// Named slots and slot forwarding
    NamedSlots,
    /// `match` elements
    Match,
    /// `MouseCursor.custom`
    CustomMouseCursor,
    /// `ComponentContainer`, `component-factory`, and the internal builtin types
    ComponentContainer,
    /// The generated Rust `new_with_existing_window` and `new_with_context` constructors
    NewWithExistingWindow,
}

/// A set of [`ExperimentalFeature`]s.
#[derive(Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(from = "Vec<ExperimentalFeature>")]
pub struct ExperimentalFeatures(u32);

impl ExperimentalFeatures {
    pub fn all() -> Self {
        ExperimentalFeature::iter().collect()
    }

    pub fn contains(self, feature: ExperimentalFeature) -> bool {
        self.0 & Self::bit(feature) != 0
    }

    fn insert(&mut self, feature: ExperimentalFeature) {
        self.0 |= Self::bit(feature);
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = ExperimentalFeature> {
        ExperimentalFeature::iter().filter(move |feature| self.contains(*feature))
    }

    /// Collects the known names, skipping the others.
    pub fn from_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> Self {
        names.into_iter().filter_map(|name| name.as_ref().parse().ok()).collect()
    }

    /// Parses a comma-separated list of feature names, such as `interfaces,match`.
    /// Returns the known features and the unknown names.
    pub fn parse_list(list: &str) -> (Self, Vec<String>) {
        let mut features = Self::default();
        let mut unknown = Vec::new();
        for name in list.split(',').map(str::trim).filter(|name| !name.is_empty()) {
            match name.parse() {
                Ok(feature) => features.insert(feature),
                Err(_) => unknown.push(name.to_string()),
            }
        }
        (features, unknown)
    }

    fn bit(feature: ExperimentalFeature) -> u32 {
        1 << feature as u32
    }
}

impl std::ops::BitOrAssign for ExperimentalFeatures {
    fn bitor_assign(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

impl FromIterator<ExperimentalFeature> for ExperimentalFeatures {
    fn from_iter<I: IntoIterator<Item = ExperimentalFeature>>(iter: I) -> Self {
        let mut features = Self::default();
        iter.into_iter().for_each(|feature| features.insert(feature));
        features
    }
}

impl From<Vec<ExperimentalFeature>> for ExperimentalFeatures {
    fn from(features: Vec<ExperimentalFeature>) -> Self {
        features.into_iter().collect()
    }
}

/// Formats as the comma-separated list that [`ExperimentalFeatures::parse_list`] reads.
impl std::fmt::Display for ExperimentalFeatures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.iter().format(","))
    }
}

impl std::fmt::Debug for ExperimentalFeatures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[test]
fn parse_list() {
    use ExperimentalFeature::*;
    let (features, unknown) = ExperimentalFeatures::parse_list(" match, named-slots,,1");
    assert_eq!(features, [Match, NamedSlots].into_iter().collect());
    assert_eq!(features.to_string(), "named-slots,match");
    assert_eq!(unknown, ["1"]);
    assert_eq!(ExperimentalFeatures::parse_list(""), (ExperimentalFeatures::default(), Vec::new()));
    let all = ExperimentalFeatures::all();
    assert_eq!(ExperimentalFeatures::parse_list(&all.to_string()), (all, Vec::new()));
}

#[test]
fn deserialize() {
    use ExperimentalFeature::*;
    let features: ExperimentalFeatures =
        serde_json::from_str(r#"["closures", "interfaces"]"#).unwrap();
    assert_eq!(features, [Interfaces, Closures].into_iter().collect());
    assert!(serde_json::from_str::<ExperimentalFeatures>("true").is_err());
    assert!(serde_json::from_str::<ExperimentalFeatures>(r#"["all"]"#).is_err());
}
