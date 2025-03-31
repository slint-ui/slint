/* Copyright © 2021 rp-rs organization
 SPDX-License-Identifier: MIT OR Apache-2.0
 Copied from https://github.com/rp-rs/rp-hal-boards/blob/main/boards/rp-pico/src/lib.rs
 and slightly changed for RP2350 chip
*/

//! A Hardware Abstraction Layer for the Raspberry Pi Pico.
//!
//! This crate serves as a HAL (Hardware Abstraction Layer) for the Raspberry Pi Pico. Since the Raspberry Pi Pico
//! is based on the RP2350 chip, it re-exports the [rp235x_hal] crate which contains the tooling to work with the
//! rp2350 chip.
//!
//! # Examples:
//!
//! The following example turns on the onboard LED. Note that most of the logic works through the [rp235x_hal] crate.
//! ```ignore
//! #![no_main]
//! use rp_pico::entry;
//! use panic_halt as _;
//! use embedded_hal::digital::v2::OutputPin;
//! use rp_pico::hal::pac;
//! use rp_pico::hal;

//! #[entry]
//! fn does_not_have_to_be_main() -> ! {
//!   let mut pac = pac::Peripherals::take().unwrap();
//!   let sio = hal::Sio::new(pac.SIO);
//!   let pins = rp_pico::Pins::new(
//!        pac.IO_BANK0,
//!        pac.PADS_BANK0,
//!        sio.gpio_bank0,
//!        &mut pac.RESETS,
//!   );
//!   let mut led_pin = pins.led.into_push_pull_output();
//!   led_pin.set_high().unwrap();
//!   loop {
//!   }
//! }
//! ```

pub extern crate rp235x_hal as hal;

extern crate cortex_m_rt;

/// The `entry` macro declares the starting function to the linker.
/// This is similar to the `main` function in console applications.
///
/// It is based on the [cortex_m_rt](https://docs.rs/cortex-m-rt/latest/cortex_m_rt/attr.entry.html) crate.
///
/// # Examples
/// ```ignore
/// #![no_std]
/// #![no_main]
/// use rp_pico::entry;
/// #[entry]
/// fn you_can_use_a_custom_main_name_here() -> ! {
///   loop {}
/// }
/// ```
#[allow(unused_imports)]
pub use hal::entry;

#[unsafe(link_section = ".start_block")]
#[unsafe(no_mangle)]
#[used]
pub static BOOT2_FIRMWARE: hal::block::ImageDef = hal::block::ImageDef::secure_exe();

#[allow(unused_imports)]
pub use hal::pac;

hal::bsp_pins!(
    /// GPIO 0 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 RX`    | [crate::Gp0Spi0Rx]          |
    /// | `UART0 TX`   | [crate::Gp0Uart0Tx]         |
    /// | `I2C0 SDA`   | [crate::Gp0I2C0Sda]         |
    /// | `PWM0 A`     | [crate::Gp0Pwm0A]           |
    /// | `PIO0`       | [crate::Gp0Pio0]            |
    /// | `PIO1`       | [crate::Gp0Pio1]            |
    Gpio0 {
        name: gpio0,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio0].
            FunctionUart, PullNone: Gp0Uart0Tx,
            /// SPI Function alias for pin [crate::Pins::gpio0].
            FunctionSpi, PullNone: Gp0Spi0Rx,
            /// I2C Function alias for pin [crate::Pins::gpio0].
            FunctionI2C, PullUp: Gp0I2C0Sda,
            /// PWM Function alias for pin [crate::Pins::gpio0].
            FunctionPwm, PullNone: Gp0Pwm0A,
            /// PIO0 Function alias for pin [crate::Pins::gpio0].
            FunctionPio0, PullNone: Gp0Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio0].
            FunctionPio1, PullNone: Gp0Pio1
        }
    },

    /// GPIO 1 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 CSn`   | [crate::Gp1Spi0Csn]         |
    /// | `UART0 RX`   | [crate::Gp1Uart0Rx]         |
    /// | `I2C0 SCL`   | [crate::Gp1I2C0Scl]         |
    /// | `PWM0 B`     | [crate::Gp1Pwm0B]           |
    /// | `PIO0`       | [crate::Gp1Pio0]            |
    /// | `PIO1`       | [crate::Gp1Pio1]            |
    Gpio1 {
        name: gpio1,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio1].
            FunctionUart, PullNone: Gp1Uart0Rx,
            /// SPI Function alias for pin [crate::Pins::gpio1].
            FunctionSpi, PullNone: Gp1Spi0Csn,
            /// I2C Function alias for pin [crate::Pins::gpio1].
            FunctionI2C, PullUp: Gp1I2C0Scl,
            /// PWM Function alias for pin [crate::Pins::gpio1].
            FunctionPwm, PullNone: Gp1Pwm0B,
            /// PIO0 Function alias for pin [crate::Pins::gpio1].
            FunctionPio0, PullNone: Gp1Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio1].
            FunctionPio1, PullNone: Gp1Pio1
        }
    },

    /// GPIO 2 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 SCK`   | [crate::Gp2Spi0Sck]         |
    /// | `UART0 CTS`  | [crate::Gp2Uart0Cts]        |
    /// | `I2C1 SDA`   | [crate::Gp2I2C1Sda]         |
    /// | `PWM1 A`     | [crate::Gp2Pwm1A]           |
    /// | `PIO0`       | [crate::Gp2Pio0]            |
    /// | `PIO1`       | [crate::Gp2Pio1]            |
    Gpio2 {
        name: gpio2,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio2].
            FunctionUart, PullNone: Gp2Uart0Cts,
            /// SPI Function alias for pin [crate::Pins::gpio2].
            FunctionSpi, PullNone: Gp2Spi0Sck,
            /// I2C Function alias for pin [crate::Pins::gpio2].
            FunctionI2C, PullUp: Gp2I2C1Sda,
            /// PWM Function alias for pin [crate::Pins::gpio2].
            FunctionPwm, PullNone: Gp2Pwm1A,
            /// PIO0 Function alias for pin [crate::Pins::gpio2].
            FunctionPio0, PullNone: Gp2Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio2].
            FunctionPio1, PullNone: Gp2Pio1
        }
    },

    /// GPIO 3 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 TX`    | [crate::Gp3Spi0Tx]          |
    /// | `UART0 RTS`  | [crate::Gp3Uart0Rts]        |
    /// | `I2C1 SCL`   | [crate::Gp3I2C1Scl]         |
    /// | `PWM1 B`     | [crate::Gp3Pwm1B]           |
    /// | `PIO0`       | [crate::Gp3Pio0]            |
    /// | `PIO1`       | [crate::Gp3Pio1]            |
    Gpio3 {
        name: gpio3,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio3].
            FunctionUart, PullNone: Gp3Uart0Rts,
            /// SPI Function alias for pin [crate::Pins::gpio3].
            FunctionSpi, PullNone: Gp3Spi0Tx,
            /// I2C Function alias for pin [crate::Pins::gpio3].
            FunctionI2C, PullUp: Gp3I2C1Scl,
            /// PWM Function alias for pin [crate::Pins::gpio3].
            FunctionPwm, PullNone: Gp3Pwm1B,
            /// PIO0 Function alias for pin [crate::Pins::gpio3].
            FunctionPio0, PullNone: Gp3Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio3].
            FunctionPio1, PullNone: Gp3Pio1
        }
    },

    /// GPIO 4 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 RX`    | [crate::Gp4Spi0Rx]          |
    /// | `UART1 TX`   | [crate::Gp4Uart1Tx]         |
    /// | `I2C0 SDA`   | [crate::Gp4I2C0Sda]         |
    /// | `PWM2 A`     | [crate::Gp4Pwm2A]           |
    /// | `PIO0`       | [crate::Gp4Pio0]            |
    /// | `PIO1`       | [crate::Gp4Pio1]            |
    Gpio4 {
        name: gpio4,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio4].
            FunctionUart, PullNone: Gp4Uart1Tx,
            /// SPI Function alias for pin [crate::Pins::gpio4].
            FunctionSpi, PullNone: Gp4Spi0Rx,
            /// I2C Function alias for pin [crate::Pins::gpio4].
            FunctionI2C, PullUp: Gp4I2C0Sda,
            /// PWM Function alias for pin [crate::Pins::gpio4].
            FunctionPwm, PullNone: Gp4Pwm2A,
            /// PIO0 Function alias for pin [crate::Pins::gpio4].
            FunctionPio0, PullNone: Gp4Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio4].
            FunctionPio1, PullNone: Gp4Pio1
        }
    },

    /// GPIO 5 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 CSn`   | [crate::Gp5Spi0Csn]         |
    /// | `UART1 RX`   | [crate::Gp5Uart1Rx]         |
    /// | `I2C0 SCL`   | [crate::Gp5I2C0Scl]         |
    /// | `PWM2 B`     | [crate::Gp5Pwm2B]           |
    /// | `PIO0`       | [crate::Gp5Pio0]            |
    /// | `PIO1`       | [crate::Gp5Pio1]            |
    Gpio5 {
        name: gpio5,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio5].
            FunctionUart, PullNone: Gp5Uart1Rx,
            /// SPI Function alias for pin [crate::Pins::gpio5].
            FunctionSpi, PullNone: Gp5Spi0Csn,
            /// I2C Function alias for pin [crate::Pins::gpio5].
            FunctionI2C, PullUp: Gp5I2C0Scl,
            /// PWM Function alias for pin [crate::Pins::gpio5].
            FunctionPwm, PullNone: Gp5Pwm2B,
            /// PIO0 Function alias for pin [crate::Pins::gpio5].
            FunctionPio0, PullNone: Gp5Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio5].
            FunctionPio1, PullNone: Gp5Pio1
        }
    },

    /// GPIO 6 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 SCK`   | [crate::Gp6Spi0Sck]         |
    /// | `UART1 CTS`  | [crate::Gp6Uart1Cts]        |
    /// | `I2C1 SDA`   | [crate::Gp6I2C1Sda]         |
    /// | `PWM3 A`     | [crate::Gp6Pwm3A]           |
    /// | `PIO0`       | [crate::Gp6Pio0]            |
    /// | `PIO1`       | [crate::Gp6Pio1]            |
    Gpio6 {
        name: gpio6,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio6].
            FunctionUart, PullNone: Gp6Uart1Cts,
            /// SPI Function alias for pin [crate::Pins::gpio6].
            FunctionSpi, PullNone: Gp6Spi0Sck,
            /// I2C Function alias for pin [crate::Pins::gpio6].
            FunctionI2C, PullUp: Gp6I2C1Sda,
            /// PWM Function alias for pin [crate::Pins::gpio6].
            FunctionPwm, PullNone: Gp6Pwm3A,
            /// PIO0 Function alias for pin [crate::Pins::gpio6].
            FunctionPio0, PullNone: Gp6Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio6].
            FunctionPio1, PullNone: Gp6Pio1
        }
    },

    /// GPIO 7 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 TX`    | [crate::Gp7Spi0Tx]          |
    /// | `UART1 RTS`  | [crate::Gp7Uart1Rts]        |
    /// | `I2C1 SCL`   | [crate::Gp7I2C1Scl]         |
    /// | `PWM3 B`     | [crate::Gp7Pwm3B]           |
    /// | `PIO0`       | [crate::Gp7Pio0]            |
    /// | `PIO1`       | [crate::Gp7Pio1]            |
    Gpio7 {
        name: gpio7,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio7].
            FunctionUart, PullNone: Gp7Uart1Rts,
            /// SPI Function alias for pin [crate::Pins::gpio7].
            FunctionSpi, PullNone: Gp7Spi0Tx,
            /// I2C Function alias for pin [crate::Pins::gpio7].
            FunctionI2C, PullUp: Gp7I2C1Scl,
            /// PWM Function alias for pin [crate::Pins::gpio7].
            FunctionPwm, PullNone: Gp7Pwm3B,
            /// PIO0 Function alias for pin [crate::Pins::gpio7].
            FunctionPio0, PullNone: Gp7Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio7].
            FunctionPio1, PullNone: Gp7Pio1
        }
    },

    /// GPIO 8 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 RX`    | [crate::Gp8Spi1Rx]          |
    /// | `UART1 TX`   | [crate::Gp8Uart1Tx]         |
    /// | `I2C0 SDA`   | [crate::Gp8I2C0Sda]         |
    /// | `PWM4 A`     | [crate::Gp8Pwm4A]           |
    /// | `PIO0`       | [crate::Gp8Pio0]            |
    /// | `PIO1`       | [crate::Gp8Pio1]            |
    Gpio8 {
        name: gpio8,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio8].
            FunctionUart, PullNone: Gp8Uart1Tx,
            /// SPI Function alias for pin [crate::Pins::gpio8].
            FunctionSpi, PullNone: Gp8Spi1Rx,
            /// I2C Function alias for pin [crate::Pins::gpio8].
            FunctionI2C, PullUp: Gp8I2C0Sda,
            /// PWM Function alias for pin [crate::Pins::gpio8].
            FunctionPwm, PullNone: Gp8Pwm4A,
            /// PIO0 Function alias for pin [crate::Pins::gpio8].
            FunctionPio0, PullNone: Gp8Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio8].
            FunctionPio1, PullNone: Gp8Pio1
        }
    },

    /// GPIO 9 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 CSn`   | [crate::Gp9Spi1Csn]         |
    /// | `UART1 RX`   | [crate::Gp9Uart1Rx]         |
    /// | `I2C0 SCL`   | [crate::Gp9I2C0Scl]         |
    /// | `PWM4 B`     | [crate::Gp9Pwm4B]           |
    /// | `PIO0`       | [crate::Gp9Pio0]            |
    /// | `PIO1`       | [crate::Gp9Pio1]            |
    Gpio9 {
        name: gpio9,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio9].
            FunctionUart, PullNone: Gp9Uart1Rx,
            /// SPI Function alias for pin [crate::Pins::gpio9].
            FunctionSpi, PullNone: Gp9Spi1Csn,
            /// I2C Function alias for pin [crate::Pins::gpio9].
            FunctionI2C, PullUp: Gp9I2C0Scl,
            /// PWM Function alias for pin [crate::Pins::gpio9].
            FunctionPwm, PullNone: Gp9Pwm4B,
            /// PIO0 Function alias for pin [crate::Pins::gpio9].
            FunctionPio0, PullNone: Gp9Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio9].
            FunctionPio1, PullNone: Gp9Pio1
        }
    },

    /// GPIO 10 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 SCK`   | [crate::Gp10Spi1Sck]        |
    /// | `UART1 CTS`  | [crate::Gp10Uart1Cts]       |
    /// | `I2C1 SDA`   | [crate::Gp10I2C1Sda]        |
    /// | `PWM5 A`     | [crate::Gp10Pwm5A]          |
    /// | `PIO0`       | [crate::Gp10Pio0]           |
    /// | `PIO1`       | [crate::Gp10Pio1]           |
    Gpio10 {
        name: gpio10,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio10].
            FunctionUart, PullNone: Gp10Uart1Cts,
            /// SPI Function alias for pin [crate::Pins::gpio10].
            FunctionSpi, PullNone: Gp10Spi1Sck,
            /// I2C Function alias for pin [crate::Pins::gpio10].
            FunctionI2C, PullUp: Gp10I2C1Sda,
            /// PWM Function alias for pin [crate::Pins::gpio10].
            FunctionPwm, PullNone: Gp10Pwm5A,
            /// PIO0 Function alias for pin [crate::Pins::gpio10].
            FunctionPio0, PullNone: Gp10Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio10].
            FunctionPio1, PullNone: Gp10Pio1
        }
    },

    /// GPIO 11 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 TX`    | [crate::Gp11Spi1Tx]         |
    /// | `UART1 RTS`  | [crate::Gp11Uart1Rts]       |
    /// | `I2C1 SCL`   | [crate::Gp11I2C1Scl]        |
    /// | `PWM5 B`     | [crate::Gp11Pwm5B]          |
    /// | `PIO0`       | [crate::Gp11Pio0]           |
    /// | `PIO1`       | [crate::Gp11Pio1]           |
    Gpio11 {
        name: gpio11,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio11].
            FunctionUart, PullNone: Gp11Uart1Rts,
            /// SPI Function alias for pin [crate::Pins::gpio11].
            FunctionSpi, PullNone: Gp11Spi1Tx,
            /// I2C Function alias for pin [crate::Pins::gpio11].
            FunctionI2C, PullUp: Gp11I2C1Scl,
            /// PWM Function alias for pin [crate::Pins::gpio11].
            FunctionPwm, PullNone: Gp11Pwm5B,
            /// PIO0 Function alias for pin [crate::Pins::gpio11].
            FunctionPio0, PullNone: Gp11Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio11].
            FunctionPio1, PullNone: Gp11Pio1
        }
    },

    /// GPIO 12 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 RX`    | [crate::Gp12Spi1Rx]         |
    /// | `UART0 TX`   | [crate::Gp12Uart0Tx]        |
    /// | `I2C0 SDA`   | [crate::Gp12I2C0Sda]        |
    /// | `PWM6 A`     | [crate::Gp12Pwm6A]          |
    /// | `PIO0`       | [crate::Gp12Pio0]           |
    /// | `PIO1`       | [crate::Gp12Pio1]           |
    Gpio12 {
        name: gpio12,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio12].
            FunctionUart, PullNone: Gp12Uart0Tx,
            /// SPI Function alias for pin [crate::Pins::gpio12].
            FunctionSpi, PullNone: Gp12Spi1Rx,
            /// I2C Function alias for pin [crate::Pins::gpio12].
            FunctionI2C, PullUp: Gp12I2C0Sda,
            /// PWM Function alias for pin [crate::Pins::gpio12].
            FunctionPwm, PullNone: Gp12Pwm6A,
            /// PIO0 Function alias for pin [crate::Pins::gpio12].
            FunctionPio0, PullNone: Gp12Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio12].
            FunctionPio1, PullNone: Gp12Pio1
        }
    },

    /// GPIO 13 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 CSn`   | [crate::Gp13Spi1Csn]        |
    /// | `UART0 RX`   | [crate::Gp13Uart0Rx]        |
    /// | `I2C0 SCL`   | [crate::Gp13I2C0Scl]        |
    /// | `PWM6 B`     | [crate::Gp13Pwm6B]          |
    /// | `PIO0`       | [crate::Gp13Pio0]           |
    /// | `PIO1`       | [crate::Gp13Pio1]           |
    Gpio13 {
        name: gpio13,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio13].
            FunctionUart, PullNone: Gp13Uart0Rx,
            /// SPI Function alias for pin [crate::Pins::gpio13].
            FunctionSpi, PullNone: Gp13Spi1Csn,
            /// I2C Function alias for pin [crate::Pins::gpio13].
            FunctionI2C, PullUp: Gp13I2C0Scl,
            /// PWM Function alias for pin [crate::Pins::gpio13].
            FunctionPwm, PullNone: Gp13Pwm6B,
            /// PIO0 Function alias for pin [crate::Pins::gpio13].
            FunctionPio0, PullNone: Gp13Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio13].
            FunctionPio1, PullNone: Gp13Pio1
        }
    },

    /// GPIO 14 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 SCK`   | [crate::Gp14Spi1Sck]        |
    /// | `UART0 CTS`  | [crate::Gp14Uart0Cts]       |
    /// | `I2C1 SDA`   | [crate::Gp14I2C1Sda]        |
    /// | `PWM7 A`     | [crate::Gp14Pwm7A]          |
    /// | `PIO0`       | [crate::Gp14Pio0]           |
    /// | `PIO1`       | [crate::Gp14Pio1]           |
    Gpio14 {
        name: gpio14,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio14].
            FunctionUart, PullNone: Gp14Uart0Cts,
            /// SPI Function alias for pin [crate::Pins::gpio14].
            FunctionSpi, PullNone: Gp14Spi1Sck,
            /// I2C Function alias for pin [crate::Pins::gpio14].
            FunctionI2C, PullUp: Gp14I2C1Sda,
            /// PWM Function alias for pin [crate::Pins::gpio14].
            FunctionPwm, PullNone: Gp14Pwm7A,
            /// PIO0 Function alias for pin [crate::Pins::gpio14].
            FunctionPio0, PullNone: Gp14Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio14].
            FunctionPio1, PullNone: Gp14Pio1
        }
    },

    /// GPIO 15 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 TX`    | [crate::Gp15Spi1Tx]         |
    /// | `UART0 RTS`  | [crate::Gp15Uart0Rts]       |
    /// | `I2C1 SCL`   | [crate::Gp15I2C1Scl]        |
    /// | `PWM7 B`     | [crate::Gp15Pwm7B]          |
    /// | `PIO0`       | [crate::Gp15Pio0]           |
    /// | `PIO1`       | [crate::Gp15Pio1]           |
    Gpio15 {
        name: gpio15,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio15].
            FunctionUart, PullNone: Gp15Uart0Rts,
            /// SPI Function alias for pin [crate::Pins::gpio15].
            FunctionSpi, PullNone: Gp15Spi1Tx,
            /// I2C Function alias for pin [crate::Pins::gpio15].
            FunctionI2C, PullUp: Gp15I2C1Scl,
            /// PWM Function alias for pin [crate::Pins::gpio15].
            FunctionPwm, PullNone: Gp15Pwm7B,
            /// PIO0 Function alias for pin [crate::Pins::gpio15].
            FunctionPio0, PullNone: Gp15Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio15].
            FunctionPio1, PullNone: Gp15Pio1
        }
    },

    /// GPIO 16 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 RX`    | [crate::Gp16Spi0Rx]         |
    /// | `UART0 TX`   | [crate::Gp16Uart0Tx]        |
    /// | `I2C0 SDA`   | [crate::Gp16I2C0Sda]        |
    /// | `PWM0 A`     | [crate::Gp16Pwm0A]          |
    /// | `PIO0`       | [crate::Gp16Pio0]           |
    /// | `PIO1`       | [crate::Gp16Pio1]           |
    Gpio16 {
        name: gpio16,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio16].
            FunctionUart, PullNone: Gp16Uart0Tx,
            /// SPI Function alias for pin [crate::Pins::gpio16].
            FunctionSpi, PullNone: Gp16Spi0Rx,
            /// I2C Function alias for pin [crate::Pins::gpio16].
            FunctionI2C, PullUp: Gp16I2C0Sda,
            /// PWM Function alias for pin [crate::Pins::gpio16].
            FunctionPwm, PullNone: Gp16Pwm0A,
            /// PIO0 Function alias for pin [crate::Pins::gpio16].
            FunctionPio0, PullNone: Gp16Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio16].
            FunctionPio1, PullNone: Gp16Pio1
        }
    },

    /// GPIO 17 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 CSn`   | [crate::Gp17Spi0Csn]        |
    /// | `UART0 RX`   | [crate::Gp17Uart0Rx]        |
    /// | `I2C0 SCL`   | [crate::Gp17I2C0Scl]        |
    /// | `PWM0 B`     | [crate::Gp17Pwm0B]          |
    /// | `PIO0`       | [crate::Gp17Pio0]           |
    /// | `PIO1`       | [crate::Gp17Pio1]           |
    Gpio17 {
        name: gpio17,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio17].
            FunctionUart, PullNone: Gp17Uart0Rx,
            /// SPI Function alias for pin [crate::Pins::gpio17].
            FunctionSpi, PullNone: Gp17Spi0Csn,
            /// I2C Function alias for pin [crate::Pins::gpio17].
            FunctionI2C, PullUp: Gp17I2C0Scl,
            /// PWM Function alias for pin [crate::Pins::gpio17].
            FunctionPwm, PullNone: Gp17Pwm0B,
            /// PIO0 Function alias for pin [crate::Pins::gpio17].
            FunctionPio0, PullNone: Gp17Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio17].
            FunctionPio1, PullNone: Gp17Pio1
        }
    },

    /// GPIO 18 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 SCK`   | [crate::Gp18Spi0Sck]        |
    /// | `UART0 CTS`  | [crate::Gp18Uart0Cts]       |
    /// | `I2C1 SDA`   | [crate::Gp18I2C1Sda]        |
    /// | `PWM1 A`     | [crate::Gp18Pwm1A]          |
    /// | `PIO0`       | [crate::Gp18Pio0]           |
    /// | `PIO1`       | [crate::Gp18Pio1]           |
    Gpio18 {
        name: gpio18,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio18].
            FunctionUart, PullNone: Gp18Uart0Cts,
            /// SPI Function alias for pin [crate::Pins::gpio18].
            FunctionSpi, PullNone: Gp18Spi0Sck,
            /// I2C Function alias for pin [crate::Pins::gpio18].
            FunctionI2C, PullUp: Gp18I2C1Sda,
            /// PWM Function alias for pin [crate::Pins::gpio18].
            FunctionPwm, PullNone: Gp18Pwm1A,
            /// PIO0 Function alias for pin [crate::Pins::gpio18].
            FunctionPio0, PullNone: Gp18Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio18].
            FunctionPio1, PullNone: Gp18Pio1
        }
    },

    /// GPIO 19 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 TX`    | [crate::Gp19Spi0Tx]         |
    /// | `UART0 RTS`  | [crate::Gp19Uart0Rts]       |
    /// | `I2C1 SCL`   | [crate::Gp19I2C1Scl]        |
    /// | `PWM1 B`     | [crate::Gp19Pwm1B]          |
    /// | `PIO0`       | [crate::Gp19Pio0]           |
    /// | `PIO1`       | [crate::Gp19Pio1]           |
    Gpio19 {
        name: gpio19,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio19].
            FunctionUart, PullNone: Gp19Uart0Rts,
            /// SPI Function alias for pin [crate::Pins::gpio19].
            FunctionSpi, PullNone: Gp19Spi0Tx,
            /// I2C Function alias for pin [crate::Pins::gpio19].
            FunctionI2C, PullUp: Gp19I2C1Scl,
            /// PWM Function alias for pin [crate::Pins::gpio19].
            FunctionPwm, PullNone: Gp19Pwm1B,
            /// PIO0 Function alias for pin [crate::Pins::gpio19].
            FunctionPio0, PullNone: Gp19Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio19].
            FunctionPio1, PullNone: Gp19Pio1
        }
    },

    /// GPIO 20 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 RX`    | [crate::Gp20Spi0Rx]         |
    /// | `UART1 TX`   | [crate::Gp20Uart1Tx]        |
    /// | `I2C0 SDA`   | [crate::Gp20I2C0Sda]        |
    /// | `PWM2 A`     | [crate::Gp20Pwm2A]          |
    /// | `PIO0`       | [crate::Gp20Pio0]           |
    /// | `PIO1`       | [crate::Gp20Pio1]           |
    Gpio20 {
        name: gpio20,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio20].
            FunctionUart, PullNone: Gp20Uart1Tx,
            /// SPI Function alias for pin [crate::Pins::gpio20].
            FunctionSpi, PullNone: Gp20Spi0Rx,
            /// I2C Function alias for pin [crate::Pins::gpio20].
            FunctionI2C, PullUp: Gp20I2C0Sda,
            /// PWM Function alias for pin [crate::Pins::gpio20].
            FunctionPwm, PullNone: Gp20Pwm2A,
            /// PIO0 Function alias for pin [crate::Pins::gpio20].
            FunctionPio0, PullNone: Gp20Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio20].
            FunctionPio1, PullNone: Gp20Pio1
        }
    },

    /// GPIO 21 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 CSn`   | [crate::Gp21Spi0Csn]        |
    /// | `UART1 RX`   | [crate::Gp21Uart1Rx]        |
    /// | `I2C0 SCL`   | [crate::Gp21I2C0Scl]        |
    /// | `PWM2 B`     | [crate::Gp21Pwm2B]          |
    /// | `PIO0`       | [crate::Gp21Pio0]           |
    /// | `PIO1`       | [crate::Gp21Pio1]           |
    Gpio21 {
        name: gpio21,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio21].
            FunctionUart, PullNone: Gp21Uart1Rx,
            /// SPI Function alias for pin [crate::Pins::gpio21].
            FunctionSpi, PullNone: Gp21Spi0Csn,
            /// I2C Function alias for pin [crate::Pins::gpio21].
            FunctionI2C, PullUp: Gp21I2C0Scl,
            /// PWM Function alias for pin [crate::Pins::gpio21].
            FunctionPwm, PullNone: Gp21Pwm2B,
            /// PIO0 Function alias for pin [crate::Pins::gpio21].
            FunctionPio0, PullNone: Gp21Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio21].
            FunctionPio1, PullNone: Gp21Pio1
        }
    },

    /// GPIO 22 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI0 SCK`   | [crate::Gp22Spi0Sck]        |
    /// | `UART1 CTS`  | [crate::Gp22Uart1Cts]       |
    /// | `I2C1 SDA`   | [crate::Gp22I2C1Sda]        |
    /// | `PWM3 A`     | [crate::Gp22Pwm3A]          |
    /// | `PIO0`       | [crate::Gp22Pio0]           |
    /// | `PIO1`       | [crate::Gp22Pio1]           |
    Gpio22 {
        name: gpio22,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio22].
            FunctionUart, PullNone: Gp22Uart1Cts,
            /// SPI Function alias for pin [crate::Pins::gpio22].
            FunctionSpi, PullNone: Gp22Spi0Sck,
            /// I2C Function alias for pin [crate::Pins::gpio22].
            FunctionI2C, PullUp: Gp22I2C1Sda,
            /// PWM Function alias for pin [crate::Pins::gpio22].
            FunctionPwm, PullNone: Gp22Pwm3A,
            /// PIO0 Function alias for pin [crate::Pins::gpio22].
            FunctionPio0, PullNone: Gp22Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio22].
            FunctionPio1, PullNone: Gp22Pio1
        }
    },

    /// GPIO 23 is connected to b_power_save of the Raspberry Pi Pico board.
    Gpio23 {
        name: b_power_save,
    },

    /// GPIO 24 is connected to vbus_detect of the Raspberry Pi Pico board.
    Gpio24 {
        name: vbus_detect,
    },

    /// GPIO 25 is connected to led of the Raspberry Pi Pico board.
    Gpio25 {
        name: led,
    },

    /// GPIO 26 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 SCK`   | [crate::Gp26Spi1Sck]        |
    /// | `UART1 CTS`  | [crate::Gp26Uart1Cts]       |
    /// | `I2C1 SDA`   | [crate::Gp26I2C1Sda]        |
    /// | `PWM5 A`     | [crate::Gp26Pwm5A]          |
    /// | `PIO0`       | [crate::Gp26Pio0]           |
    /// | `PIO1`       | [crate::Gp26Pio1]           |
    Gpio26 {
        name: gpio26,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio26].
            FunctionUart, PullNone: Gp26Uart1Cts,
            /// SPI Function alias for pin [crate::Pins::gpio26].
            FunctionSpi, PullNone: Gp26Spi1Sck,
            /// I2C Function alias for pin [crate::Pins::gpio26].
            FunctionI2C, PullUp: Gp26I2C1Sda,
            /// PWM Function alias for pin [crate::Pins::gpio26].
            FunctionPwm, PullNone: Gp26Pwm5A,
            /// PIO0 Function alias for pin [crate::Pins::gpio26].
            FunctionPio0, PullNone: Gp26Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio26].
            FunctionPio1, PullNone: Gp26Pio1
        }
    },

    /// GPIO 27 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 TX`    | [crate::Gp27Spi1Tx]         |
    /// | `UART1 RTS`  | [crate::Gp27Uart1Rts]       |
    /// | `I2C1 SCL`   | [crate::Gp27I2C1Scl]        |
    /// | `PWM5 B`     | [crate::Gp27Pwm5B]          |
    /// | `PIO0`       | [crate::Gp27Pio0]           |
    /// | `PIO1`       | [crate::Gp27Pio1]           |
    Gpio27 {
        name: gpio27,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio27].
            FunctionUart, PullNone: Gp27Uart1Rts,
            /// SPI Function alias for pin [crate::Pins::gpio27].
            FunctionSpi, PullNone: Gp27Spi1Tx,
            /// I2C Function alias for pin [crate::Pins::gpio27].
            FunctionI2C, PullUp: Gp27I2C1Scl,
            /// PWM Function alias for pin [crate::Pins::gpio27].
            FunctionPwm, PullNone: Gp27Pwm5B,
            /// PIO0 Function alias for pin [crate::Pins::gpio27].
            FunctionPio0, PullNone: Gp27Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio27].
            FunctionPio1, PullNone: Gp27Pio1
        }
    },

    /// GPIO 28 supports following functions:
    ///
    /// | Function     | Alias with applied function |
    /// |--------------|-----------------------------|
    /// | `SPI1 RX`    | [crate::Gp28Spi1Rx]         |
    /// | `UART0 TX`   | [crate::Gp28Uart0Tx]        |
    /// | `I2C0 SDA`   | [crate::Gp28I2C0Sda]        |
    /// | `PWM6 A`     | [crate::Gp28Pwm6A]          |
    /// | `PIO0`       | [crate::Gp28Pio0]           |
    /// | `PIO1`       | [crate::Gp28Pio1]           |
    Gpio28 {
        name: gpio28,
        aliases: {
            /// UART Function alias for pin [crate::Pins::gpio28].
            FunctionUart, PullNone: Gp28Uart0Tx,
            /// SPI Function alias for pin [crate::Pins::gpio28].
            FunctionSpi, PullNone: Gp28Spi1Rx,
            /// I2C Function alias for pin [crate::Pins::gpio28].
            FunctionI2C, PullUp: Gp28I2C0Sda,
            /// PWM Function alias for pin [crate::Pins::gpio28].
            FunctionPwm, PullNone: Gp28Pwm6A,
            /// PIO0 Function alias for pin [crate::Pins::gpio28].
            FunctionPio0, PullNone: Gp28Pio0,
            /// PIO1 Function alias for pin [crate::Pins::gpio28].
            FunctionPio1, PullNone: Gp28Pio1
        }
    },

    /// GPIO 29 is connected to voltage_monitor of the Raspberry Pi Pico board.
    Gpio29 {
        name: voltage_monitor,
    },
);

pub const XOSC_CRYSTAL_FREQ: u32 = 12_000_000;
