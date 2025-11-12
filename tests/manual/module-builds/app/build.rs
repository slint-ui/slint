// Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com
// SPDX-License-Identifier: MIT

fn main() {
    slint_build::compile("ui/app-window.slint").expect("Slint build failed");
}
