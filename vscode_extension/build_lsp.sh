#!/bin/bash -e
# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2020 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2020 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END

cd `dirname $0`/..
for lsp_suffix in x86_64-unknown-linux-gnu x86_64-pc-windows-gnu.exe; do
    triplet=`basename $lsp_suffix .exe`
    exe_suffix=`echo $lsp_suffix | sed -n -e "s,.*\(\.exe\),\1,p"`
    cross build --target $triplet --release -p sixtyfps-lsp
    cp target/$triplet/release/sixtyfps-lsp$exe_suffix vscode_extension/bin/sixtyfps-lsp-$suffix
done
cargo build --release -p sixtyfps-lsp
cp target/release/sixtyfps-lsp vscode_extension/bin/sixtyfps-lsp-x86_64-apple-darwin