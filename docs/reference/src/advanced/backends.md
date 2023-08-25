<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->

# Platform Backends

In Slint, a backend is the module that encapsulates the interaction with the operating system,
in particular the windowing sub-system. Multiple backends can be compiled into Slint and one
backend is selected for use at run-time on application start-up. You can configure Slint without
any built-in backends, and instead develop your own backend by implementing Slint's platform
abstraction and window adapter interfaces.

The backend is selected as follows:

1. The developer provides their own backend and sets it programmatically.
2. Else, the backend is selected by the value of the `SLINT_BACKEND` environment variable, if it is set.
3. Else, backends are tried for initialization in the following order:
   1. qt 
   2. winit
   3. linuxkms

The following table provides an overview over the built-in backends. For more information about the backend's
capabilities and their configuration options, see the respective sub-pages.

| Backend Name | Description                                                                                             | Built-in by Default   |
|--------------|---------------------------------------------------------------------------------------------------------|-----------------------|
| qt           | The Qt library is used for windowing system integration, rendering, and native widget styling.          | Yes (if Qt installed) |
| winit        | The [winit](https://docs.rs/winit/latest/winit/) library is used to interact with the windowing system. | Yes                   |
| linuxkms     | Linux's KMS/DRI infrastructure is used for rendering. No windowing system or compositor is required.    | No                    |


```{toctree}
:hidden:
:maxdepth: 2

backend_qt.md
backend_winit.md
backend_linuxkms.md
```
