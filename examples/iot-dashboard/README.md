# iot-dashboard

This example is a clone of https://github.com/uwerat/qskinny/tree/master/examples/iotdashboard from
the [QSkinny framework](https://qskinny.github.io/)

The images are originating from that repository

The `main.slint` and `iot-dashboard.slint` files are basically a pure translation from
the C++ QSkinny code to self-contained .slint.

## Online preview:

https://slint.dev/snapshots/master/editor/preview.html?load_url=https://raw.githubusercontent.com/slint-ui/slint/master/examples/iot-dashboard/main.slint

## Screenshot

![Screenshot of the IOT Dashboard](https://slint.dev/resources/iot-dashboard_screenshot.png "IOT Dashboard")

## Loading dynamic widgets from C++

The example was also extended with C++ code (the `.cpp`) to show how to use the C++
interpreter to dynamically generate .slint code on the fly and to show different
widgets and their backend, forwarding all the properties from widgets to the
root so they can be changed by the backend.
