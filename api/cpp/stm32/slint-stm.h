// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#ifndef STM_SLINT_H_
#define STM_SLINT_H_

#include <slint-platform.h>
#include <slint-stm-config.h>

struct SlintPlatformConfiguration
{
    /// The size of the screen in pixels.
    slint::PhysicalSize size
#if defined(LCD_DEFAULT_WIDTH) && defined(LCD_DEFAULT_HEIGHT)
            = slint::PhysicalSize({ LCD_DEFAULT_WIDTH, LCD_DEFAULT_HEIGHT })
#endif
            ;
    unsigned int lcd_layer_0_address
#if defined(LCD_LAYER_0_ADDRESS)
            = LCD_LAYER_0_ADDRESS
#endif
            ;
    unsigned int lcd_layer_1_address
#if defined(LCD_LAYER_0_ADDRESS)
            = LCD_LAYER_1_ADDRESS
#endif
            ;
};

void slint_stm_init(const SlintPlatformConfiguration &config);

#endif // STM_SLINT_H_
