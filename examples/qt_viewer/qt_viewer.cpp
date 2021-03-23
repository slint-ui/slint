/* LICENSE BEGIN
    This file is part of the SixtyFPS Project -- https://sixtyfps.io
    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only
    This file is also available under commercial licensing terms.
    Please contact info@sixtyfps.io for more information.
LICENSE END */

#include <QtWidgets/QtWidgets>
#include <sixtyfps_interpreter.h>

#include "ui_interface.h"


struct LoadedFile {
    sixtyfps::ComponentHandle<sixtyfps::interpreter::ComponentInstance> instance;
    QWidget *widget;
};

void show_diagnostics(QWidget *root, const sixtyfps::SharedVector< sixtyfps::interpreter::Diagnostic > &diags) {
    QString text;

    for (auto diagnostic : diags) {
        text += (diagnostic.level == sixtyfps::interpreter::DiagnosticLevel::Warning
                              ? QApplication::translate("qt_viewer", "warning: %1\n")
                              : QApplication::translate("qt_viewer", "error: %1\n")
                 ).arg(QString::fromUtf8(diagnostic.message.data()));

        text += QApplication::translate("qt_viewer", "location: %1").arg(QString::fromUtf8(diagnostic.source_file.data()));
        if (diagnostic.line > 0)
            text += ":" + QString::number(diagnostic.line);
        if (diagnostic.column > 0)
            text += ":" + QString::number(diagnostic.column);
        text += "\n";
    }

    QMessageBox::critical(root, QApplication::translate("qt_viewer", "Compilation error"), text, QMessageBox::StandardButton::Ok);
}

int main(int argc, char **argv) {
    QApplication app(argc, argv);
    std::unique_ptr<LoadedFile> loaded_file;

    QWidget main;
    Ui::Interface ui;
    ui.setupUi(&main);
    QHBoxLayout layout(ui.my_content);

    QObject::connect(ui.load_button, &QPushButton::clicked, [&] {
        QString fileName = QFileDialog::getOpenFileName(
            &main, QApplication::translate("qt_viewer", "Open SixtyFPS File"), {},
            QApplication::translate("qt_viewer", "SixtyFPS File (*.60)"));
        if (fileName.isEmpty())
            return;
        loaded_file.reset();
        sixtyfps::interpreter::ComponentCompiler compiler;
        auto def = compiler.build_from_path(fileName.toUtf8().data());
        if (!def) {
            show_diagnostics(&main, compiler.diagnostics());
            return;
        }
        auto instance = def->create();
        QWidget *wid = instance->qwidget();
        wid->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
        layout.addWidget(wid);
        loaded_file = std::make_unique<LoadedFile>(LoadedFile{ instance, wid });
    });
    main.show();
    return app.exec();
}

