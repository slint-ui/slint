<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

This shows how one can use the Slint C++ platform API to integrate into any Windows application

 - main.cpp is basically a shell of an application written using the native WIN32 api.
 - appview.h is an interface that is used by the application to show a Slint Window.
   the implementation of this interface could even be in a plugin.
 - appview.cpp is the implementation of this interface and instantiate the UI made with Slint
 - windowadapter_win.h contains the glue code used to implement a Slint platform using native WIN32 API
