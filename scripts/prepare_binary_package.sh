#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

if [ $# -lt 1 ]; then
    echo "usage: $0 path/to/target/binary_package [cargo feature flags...]"
    echo
    echo "This prepares the specified binary_package folder for distribution"
    echo "by adding the legal copyright and license notices."
    echo
    echo "All files will be copied/created under the licenses folder"
    echo "along with a THIRDPARTY.md"
    echo
    echo "Any additional arguments (e.g. --no-default-features --features ...)"
    echo "are forwarded to the license generator so that the third-party list"
    echo "matches the features the binary was built with."
    echo
    exit 1
fi

target_path=$1/licenses
script_dir=`dirname $0`
shift

mkdir -p $target_path
cp -a $script_dir/../LICENSE.md $target_path

cargo run --locked --manifest-path $script_dir/../xtask/Cargo.toml -- license \
    --manifest-path "$PWD/Cargo.toml" \
    -o $target_path/THIRDPARTY.md \
    "$@"
