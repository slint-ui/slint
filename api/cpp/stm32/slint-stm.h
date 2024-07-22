// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#define STRINGIFY(X) STRINGIFY2(X)
#define STRINGIFY2(X) #X
#define CAT(X, Y) CAT2(X, Y)
#define CAT2(X, Y) X##Y

#include <slint-platform.h>

#if !defined(SLINT_STM32_BSP_NAME)
#    error "Please define the SLINT_STM32_BSP_NAME pre-processor macro to the base name of the BSP, without quotes, such as SLINT_STM32_BSP_NAME=stm32h747i_disco"
#endif

#include STRINGIFY(SLINT_STM32_BSP_NAME.h)
#include STRINGIFY(CAT(SLINT_STM32_BSP_NAME, _lcd.h))
#include STRINGIFY(CAT(SLINT_STM32_BSP_NAME, _ts.h))

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
