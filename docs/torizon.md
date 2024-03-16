<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
# Running Slint Demos on Torizon

Toradex provides [TorizonCore](https://developer.toradex.com/torizon/) a Linux based platform for its embedded devices that packages applications in docker containers.

We provide our demos compiled for Toradex as docker containers.

## Prerequisites

 - A device running Torizon
 - A running [weston container](https://developer.toradex.com/torizon/5.0/provided-containers/working-with-weston-on-torizoncore)
 - SSH access to the Torizon device

## Running

Our pre-compiled demos are available in four different variants:

1. Compiled for ARM 32-bit as `armhf` and compiled for ARM 64-bit as `arm64`
2. Compiled with Linux DRI or with support for Vivante GPUs (`-vivante` suffix)

A complete list of all containers can be found at

https://github.com/orgs/slint-ui/packages?q=torizon&tab=packages&q=torizon

For example to run the container on an i.MX8 board with Vivante GPU, use the following command line:

```
docker run --user=torizon -v /dev:/dev -v /tmp:/tmp --device-cgroup-rule='c 199:* rmw' --device-cgroup-rule='c 226:* rmw ghcr.io/slint-ui/slint/torizon-demos-arm64-vivante
```

## Selecting Demos

By default, the printer demo from /usr/bin is run. The containers however package multiple demos:

 * printerdemo
 * slide_puzzle
 * gallery
 * opengl_underlay
 * carousel
 * todo
 * energy-monitor

Run then by specifying them as parameter to `docker run`, for example:

```
docker run --user=torizon -v /dev:/dev -v /tmp:/tmp --device-cgroup-rule='c 199:* rmw' --device-cgroup-rule='c 226:* rmw ghcr.io/slint-ui/slint/torizon-demos-arm64-vivante opengl_underlay
```
