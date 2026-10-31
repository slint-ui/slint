// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

//! The host link of a desktop simulator: a pseudo-terminal, which a program that drives a
//! device over a serial port opens like one.

extern crate std;

use alloc::string::String;

/// Creates the pseudo-terminal and prints the name of its device.
/// Slint must already use a desktop backend, so nothing else needs initializing.
pub fn init_with_host_link() -> impl crate::HostLink {
    let link = imp::PseudoTerminal::open().expect("Cannot create a pseudo-terminal");
    let name = crate::HostLink::port_name(&link);
    std::println!(
        "The simulator's serial port is {}",
        name.unwrap_or("unavailable: the simulator only has one on Linux and macOS")
    );
    link
}

#[cfg(unix)]
mod imp {
    use super::*;
    use serialport::SerialPort as _;
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};

    pub struct PseudoTerminal {
        main: serialport::TTYPort,
        name: String,
        /// Held open, so that the main side keeps working while no program has the port open.
        _subsidiary: serialport::TTYPort,
    }

    impl PseudoTerminal {
        pub fn open() -> serialport::Result<Self> {
            let (mut main, subsidiary) = serialport::TTYPort::pair()?;
            main.set_timeout(Duration::ZERO)?;
            let name = subsidiary.name().unwrap_or_default();
            Ok(Self { main, name, _subsidiary: subsidiary })
        }
    }

    impl crate::HostLink for PseudoTerminal {
        fn read(&mut self, buffer: &mut [u8]) -> usize {
            self.main.read(buffer).unwrap_or(0)
        }

        fn write(&mut self, mut bytes: &[u8]) {
            // Nobody may be reading: give up on what doesn't go out within a moment
            let deadline = Instant::now() + Duration::from_millis(100);
            while !bytes.is_empty() && Instant::now() < deadline {
                match self.main.write(bytes) {
                    Ok(written) => bytes = &bytes[written..],
                    Err(_) => std::thread::sleep(Duration::from_millis(1)),
                }
            }
        }

        fn board_name(&self) -> &str {
            "Simulator"
        }

        fn port_name(&self) -> Option<&str> {
            Some(&self.name)
        }
    }
}

#[cfg(not(unix))]
mod imp {
    use super::*;

    /// Windows has no pseudo-terminal that a serial port library opens: this one never
    /// receives anything.
    pub struct PseudoTerminal;

    impl PseudoTerminal {
        pub fn open() -> std::io::Result<Self> {
            Ok(Self)
        }
    }

    impl crate::HostLink for PseudoTerminal {
        fn read(&mut self, _buffer: &mut [u8]) -> usize {
            0
        }

        fn write(&mut self, _bytes: &[u8]) {}

        fn board_name(&self) -> &str {
            "Simulator"
        }
    }
}
