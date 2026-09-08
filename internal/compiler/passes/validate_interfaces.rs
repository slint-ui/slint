// Copyright © 2026 Klarälvdalens Datakonsult AB, a KDAB Group company <info@kdab.com>, author Nathan Collins <nathan.collins@kdab.com>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

use crate::diagnostics::BuildDiagnostics;
use crate::object_tree::Document;
use crate::object_tree::interfaces::validate_implement_statements;

pub(crate) fn validate_interfaces(doc: &Document, diag: &mut BuildDiagnostics) {
    for component in doc.inner_components.iter() {
        validate_implement_statements(&component.root_element, diag);
    }
}
