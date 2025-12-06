// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

#include <zephyr/logging/log.h>
LOG_MODULE_REGISTER(homeAutomation, LOG_LEVEL_INF);

#include <zephyr/kernel.h>
#include <zephyr/device.h>
#include <zephyr/drivers/display.h>

#ifdef CONFIG_ARCH_POSIX
#    include "posix_board_if.h"
#endif

#include <app_version.h>

#include "slint-zephyr.h"

#include "demo-sw-renderer.h"

int main(void)
{
    printk("Slint Home Automation Demo with Zephyr %s\n", APP_VERSION_STRING);

    const struct device *display_dev = nullptr;

    display_dev = DEVICE_DT_GET(DT_CHOSEN(zephyr_display));
    if (!device_is_ready(display_dev)) {
        LOG_ERR("Device %s not found. Aborting.", display_dev->name);
#ifdef CONFIG_ARCH_POSIX
        posix_exit(1);
#else
        return 0;
#endif
    }

    slint_zephyr_init(display_dev);

    auto app_window = AppWindow::create();
    app_window->run();

    return 0;
}
