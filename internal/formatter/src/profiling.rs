// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//! Development-only profiling and corpus analysis helpers for formatter experimentation.

pub use crate::style_profile::{
    FileStyleProfile, IndentationDecisionKind, IndentationObservation,
    IndentationObservationComparison, IndentationObservationSummary, RepositoryStyleComparison,
    RepositoryStyleProfile, StyleChoice, StyleDecision, StyleDecisionComparison, StyleDecisionKind,
    StyleDecisionSummary, aggregate_repository_profile, collect_standalone_slint_files,
    compare_repository_profiles, format_repository_style_comparison_report,
    format_repository_style_report, profile_file, profile_source, profile_source_with_path,
};
