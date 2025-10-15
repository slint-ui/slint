<!-- Copyright © SixtyFPS GmbH <info@slint.dev> ; SPDX-License-Identifier: MIT -->
<!-- cSpell: ignore Torizon Toradex Vivante imx8 am62 imx95 PowerVR -->
# Running Slint Demos on Torizon OS

Toradex provides [Torizon OS](https://developer.toradex.com/torizon/) a Linux based platform for its embedded devices that packages applications in docker containers.

We provide our demos compiled for Toradex as docker containers with GPU acceleration support.

## Prerequisites

- A device running Torizon OS 6.0 or later
- SSH access to the Torizon device
- Docker installed on the device

**Note**: The Slint demos run directly on the linux/kms and do not require a Weston container.

## Running

Our pre-compiled demos are available in multiple variants optimized for different hardware platforms:

1. **Standard ARM64 without GPU build** (`torizon-demos-arm64`) - Uses software rendering
2. **i.MX8 GPU build** (`torizon-demos-arm64-imx8`) - Optimized for i.MX8 series with GPU acceleration
3. **AM62 GPU build** (`torizon-demos-arm64-am62`) - Optimized for AM62 series with GPU acceleration
4. **i.MX95 GPU build** (`torizon-demos-arm64-imx95`) - Optimized for i.MX95 series with GPU acceleration
5. **Vivante GPU build** (`torizon-demos-arm64-vivante`) - Legacy variant for i.MX8 with Vivante GPU acceleration

A complete list of all containers can be found at

https://github.com/orgs/slint-ui/packages?q=torizon&tab=packages&q=torizon

### Running with Docker

For i.MX8 series boards:

```bash
sudo docker run --rm --privileged \
  --user=torizon \
  -v /dev:/dev \
  -v /tmp:/tmp \
  -v /run/udev:/run/udev \
  --device-cgroup-rule='c 199:* rmw' \
  --device-cgroup-rule='c 226:* rmw' \
  --device-cgroup-rule='c 13:* rmw' \
  --device-cgroup-rule='c 4:* rmw' \
  ghcr.io/slint-ui/slint/torizon-demos-arm64-imx8
```

For AM62 series boards with GPU:

```bash
sudo docker run --rm --privileged \
  --user=torizon \
  -v /dev:/dev \
  -v /tmp:/tmp \
  -v /run/udev:/run/udev \
  --device-cgroup-rule='c 199:* rmw' \
  --device-cgroup-rule='c 226:* rmw' \
  --device-cgroup-rule='c 13:* rmw' \
  --device-cgroup-rule='c 4:* rmw' \
  ghcr.io/slint-ui/slint/torizon-demos-arm64-am62
```

For generic ARM64 devices without GPU acceleration:

```bash
sudo docker run --rm --privileged \
  --user=torizon \
  -v /dev:/dev \
  -v /tmp:/tmp \
  -v /run/udev:/run/udev \
  --device-cgroup-rule='c 199:* rmw' \
  --device-cgroup-rule='c 226:* rmw' \
  --device-cgroup-rule='c 13:* rmw' \
  --device-cgroup-rule='c 4:* rmw' \
  ghcr.io/slint-ui/slint/torizon-demos-arm64
```

## Available Demos

By default, the **energy-monitor** demo is run. The containers package multiple demo applications:

- **energy-monitor** (default) - Energy monitoring dashboard
- **printerdemo** - 3D printer control interface
- **gallery** - Image gallery with touch navigation
- **slide_puzzle** - Interactive sliding puzzle game
- **opengl_underlay** - OpenGL rendering demonstration
- **carousel** - 3D carousel interface
- **todo** - Task management application
- **weather-demo** - Weather information display (requires API key)
- **home-automation** - Smart home control panel

### Selecting Specific Demos

Run a specific demo by specifying it as a parameter:

```bash
# Printer demo on i.MX8 with GPU acceleration
sudo docker run --rm --privileged --user=torizon \
  -v /dev:/dev -v /tmp:/tmp -v /run/udev:/run/udev \
  --device-cgroup-rule='c 199:* rmw' --device-cgroup-rule='c 226:* rmw' \
  --device-cgroup-rule='c 13:* rmw' --device-cgroup-rule='c 4:* rmw' \
  ghcr.io/slint-ui/slint/torizon-demos-arm64-imx8 printerdemo

# Todo demo on AM62 without GPU acceleration
sudo docker run --rm --privileged --user=torizon \
  -v /dev:/dev -v /tmp:/tmp -v /run/udev:/run/udev \
  --device-cgroup-rule='c 199:* rmw' --device-cgroup-rule='c 226:* rmw' \
  --device-cgroup-rule='c 13:* rmw' --device-cgroup-rule='c 4:* rmw' \
  ghcr.io/slint-ui/slint/torizon-demos-arm64 todo
```

### Auto-running on Boot

Torizon OS supports automatically starting containers on boot by placing a `docker-compose.yml` file in `/var/sota/storage/docker-compose/` on the device.

[How to Autorun an Application With Torizon OS
](https://developer.toradex.com/torizon/application-development/working-with-containers/how-to-autorun-an-application-with-torizoncore/)

**Step 1: Create docker-compose.yml**

Create a file named `docker-compose.yml` with the following content (adjust the image variant for your platform):

```yaml
services:
  slint-demo:
    image: ghcr.io/slint-ui/slint/torizon-demos-arm64-imx8:latest
    restart: unless-stopped
    environment:
      - ACCEPT_FSL_EULA=1
      - SLINT_FULLSCREEN=1
      - SLINT_BACKEND=linuxkms-skia-opengl
    user: torizon
    privileged: true
    volumes:
      - type: bind
        source: /tmp
        target: /tmp
      - type: bind
        source: /dev
        target: /dev
      - type: bind
        source: /run/udev
        target: /run/udev
    device_cgroup_rules:
      # ... for tty
      - "c 4:* rmw"
      # ... for /dev/input devices
      - "c 13:* rmw"
      - "c 199:* rmw"
      # ... for /dev/dri devices
      - "c 226:* rmw"
    command: home-automation
```

**Step 2: Deploy to Device**

Copy the file to your Torizon device:

```bash
# Create the directory if it doesn't exist
ssh torizon@<device-ip> "sudo mkdir -p /var/sota/storage/docker-compose"

# Copy the docker-compose.yml file
scp docker-compose.yml torizon@<device-ip>:/tmp/
ssh torizon@<device-ip> "sudo mv /tmp/docker-compose.yml /var/sota/storage/docker-compose/"

# Reboot to start the application automatically
ssh torizon@<device-ip> "sudo reboot"
```

**Platform-specific variants:**
- For **i.MX8**: `ghcr.io/slint-ui/slint/torizon-demos-arm64-imx8:latest`
- For **AM62 with GPU**: `ghcr.io/slint-ui/slint/torizon-demos-arm64-am62:latest`
- For **i.MX95**: `ghcr.io/slint-ui/slint/torizon-demos-arm64-imx95:latest`
- For **Generic ARM64**: `ghcr.io/slint-ui/slint/torizon-demos-arm64:latest`

The demo will automatically start on boot and restart if it crashes. To stop auto-running, remove the file:

```bash
ssh torizon@<device-ip> "sudo rm /var/sota/storage/docker-compose/docker-compose.yml && sudo reboot"
```
