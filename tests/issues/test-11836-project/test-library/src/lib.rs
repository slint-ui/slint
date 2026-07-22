// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

pub mod test_library {
    slint::include_modules!();
    use slint::ComponentHandle;

    pub fn init<T>(_ui: &T)
    where
        T: ComponentHandle + 'static,
        for<'a> TestGlobal<'a>: slint::Global<'a, T>,
    {
    }
}
