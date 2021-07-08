# iot-dashboard

This example is a clone of https://github.com/uwerat/qskinny/tree/master/examples/iotdashboard from
the [QSkinny framework](https://qskinny.github.io/)

The images are originating from that repository

The `main.60` and `iot-dashboard.60` files are basically a pure translation from
the C++ QSkinny code to self-contained .60.

## Online preview:

https://sixtyfps.io/snapshots/master/editor/preview.html?load_url=https://raw.githubusercontent.com/sixtyfpsui/sixtyfps/master/examples/iot-dashboard/main.60

## Loading dynamic widgets from C++

The example was also extended with C++ code (the `.cpp`) to show how to use the C++
interpreter to dynamically generate .60 code on the fly and to show different
widgets and their backend, forwarding all the properties from widgets to the
root so they can be changed by the backend.
