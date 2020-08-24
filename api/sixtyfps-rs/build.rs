/* LICENSE BEGIN

    This file is part of the Sixty FPS Project

    Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
    Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>

    SPDX-License-Identifier: GPL-3.0-only

LICENSE END */
use rustc_version::{version_meta, Channel};

fn main() {
    // Set cfg flags depending on release channel
    //
    match version_meta().unwrap().channel {
        Channel::Stable => println!("cargo:rustc-cfg=stable"),
        Channel::Beta => println!("cargo:rustc-cfg=beta"),
        Channel::Nightly => println!("cargo:rustc-cfg=nightly"),
        Channel::Dev => println!("cargo:rustc-cfg=rustc_dev"),
    }
}
