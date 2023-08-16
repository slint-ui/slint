<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Install Qt

TLDR; If you are redirected to this document because of a link in the warning that Qt wasn't found and
you want to silence the warning without installing Qt, you can set this environment variable: `SLINT_NO_QT=1`

## Do I need Qt to use Slint?

Short answer: No. Only if you want to use the Qt backend used for the native style.

Slint has two backends: GL and Qt. The GL backend uses the `femtovg` and `winit` crate for the rendering.
The Qt backend uses Qt. In addition, the Qt backend provide the implementation for the native widget
from the `native` style.
Qt is only needed if you want native looking widgets. Otherwise, another style will be used for widget, which does not
look native.
In the future, we plan to have native backend using the native API, which will allow native widgets without using Qt.

## How to install Qt

You will need the Qt >= 5.15

You can just download and install the latest version of Qt from https://www.qt.io/download-qt-installer or any other sources

Then simply make sure that `qmake` executable is in the `PATH` when you build Slint. The executable is
typically located in the `bin` sub-directory of a Qt installation that was produced by the Qt installer.
Alternatively, you can set the `QMAKE` environment variable to point to the `qmake` executable.
(more info: <https://docs.rs/qttypes/*/qttypes/#finding-qt> )

### Linux

Many distributions may provide Qt 5.15 in the distribution package. In that case you can install these packages
and there isn't much more to do. On many distributions, you also need the **-dev** packages. For distributions that
split the packages in different modules, you just need `qtbase` (for QtWidgets) and `qtsvg` for the SVG plugin.

If when running your Slint application you get an error that libQt5Core.so.5 or such can't be found, you need to
adjust the `LD_LIBRARY_PATH` environment variable to contain a path that contains the Qt libraries.

### macOS

In addition to either having `qmake` in your `PATH` or setting `QMAKE`, you also need to modify the `DYLD_FRAMEWORK_PATH`
environment variable. It needs to be set to the `lib` directory of your Qt installation, for example `$HOME/Qt/6.2.0/macos/lib`,
in order for the dynamic linker to find the Qt libraries when starting an application.

### Windows

For Windows it's necessary to have the `bin` directory of your Qt installation in the list of paths in the `PATH`
environment variable, in order for the build system to locate `qmake` and to find the Qt DLLs when starting an application.

## How To Disable the Qt Backend

By setting the `SLINT_NO_QT` environment variable when building Slint, the Qt backend won't be compiled and
no attempt will be made to find Qt on the system. This will also disable the warning stating that Qt wasn't found.
