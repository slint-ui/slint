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

mkdir -p bin
cd ..

suffices_to_build="x86_64-pc-windows-gnu.exe"

case $OSTYPE in
    darwin*)
        suffices_to_build="$suffices_to_build x86_64-unknown-linux-gnu"
        native_suffix="x86_64-apple-darwin"
        ;;
    linux*)
        native_suffix="x86_64-unknown-linux-gnu"
        ;;
esac

for lsp_suffix in $suffices_to_build; do
    triplet=`basename $lsp_suffix .exe`
    exe_suffix=`echo $lsp_suffix | sed -n -e "s,.*\(\.exe\),\1,p"`
    cross build --target $triplet --release -p sixtyfps-lsp
    cp target/$triplet/release/sixtyfps-lsp$exe_suffix vscode_extension/bin/sixtyfps-lsp-$lsp_suffix
done
cargo build --release -p sixtyfps-lsp
cp target/release/sixtyfps-lsp vscode_extension/bin/sixtyfps-lsp-$native_suffix