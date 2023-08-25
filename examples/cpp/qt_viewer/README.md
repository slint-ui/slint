<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# qt_viewer

This is an example that shows how to embed a dynamically loaded .slint into a Qt (QWidgets) application

The trick is that it uses the C++ `slint::interpreter::ComponentInstance::qwidget` and embed
that widget in a Qt application.
