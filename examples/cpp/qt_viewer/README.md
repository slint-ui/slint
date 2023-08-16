<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-1.1 OR LicenseRef-Slint-commercial -->
# qt_viewer

This is an example that shows how to embed a dynamically loaded .slint into a Qt (QWidgets) application

The trick is that it uses the C++ `slint::interpreter::ComponentInstance::qwidget` and embed
that widget in a Qt application.
