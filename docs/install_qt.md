# Install Qt

TLDR; If you are redirected to this document because of a link in the warning that Qt was not found and
you want to silence the warning without installing Qt, you can set this environment variable: `SIXTYFPS_NO_QT=1`

## Do I need Qt to use SixtyFPS?

Short answer: No. Only if you want to use the Qt backend used for the native style.

SixtyFPS has two backends: GL and Qt. The GL backend uses the `femtovg` and `winit` crate for the rendering.
The Qt backend uses Qt. In addition, the Qt backend provide the implementation for the native widget
from the `native` style.
Qt is only needed if you want native looking widgets. Otherwise, another style will be used for widget, which does not
look native.

You will need the Qt version 5.15.x Qt 6 is not yet supported.

## How to install Qt

You can just download and install Qt 5.15 from https://www.qt.io/download-qt-installer and any other sources

Then simply make sure that `qmake` executable is in the `PATH` when you build SixtyFP.
Alternatively, you can set the `QMAKE` environment variable to point to the `qmake` executable.

### Linux

Many distributions may contains Qt 5.15 in the distribution package. In that case you can simply install these packages
and there is not much more to do.

If when running your SixtyFPS application you get an error that libQt5Core.so.5 or such cannot be found, you need to
adjust the LD_LIBRARY_PATH environment variable to contain a path that contains the Qt library.

## How To Disable the Qt Backend

By setting the `SIXTYFPS_NO_QT` environment variable when building SixtyFPS, the Qt backend will not be compiled and
no attempt will be made to find Qt on the system. This will also disable the warning stating that Qt was not found.



