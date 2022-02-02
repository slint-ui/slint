# qt_viewer

This is an example that shows how to embed a dynamically loaded .slint into a Qt (QWidgets) application

The trick is that it uses the C++ `sixtyfps::interpreter::ComponentInstance::qwidget` and embed
that widget in a Qt application.
