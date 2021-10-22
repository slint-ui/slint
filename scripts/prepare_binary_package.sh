#!/bin/bash -e
# LICENSE BEGIN
# This file is part of the SixtyFPS Project -- https://sixtyfps.io
# Copyright (c) 2021 Olivier Goffart <olivier.goffart@sixtyfps.io>
# Copyright (c) 2021 Simon Hausmann <simon.hausmann@sixtyfps.io>
#
# SPDX-License-Identifier: GPL-3.0-only
# This file is also available under commercial licensing terms.
# Please contact info@sixtyfps.io for more information.
# LICENSE END

if [ $# != 3 ]; then
    echo "usage: $0 path/to/target/binary_package path/to/binary/Qt qt_version"
    echo
    echo "This prepares the specified binary_package folder for distribution"
    echo "by adding the legal copyright and license notices."
    echo
    echo "All files will be copied/created under the 3rdparty-licenses folder"
    echo "along with an index.html"
    echo
    echo "(The path to Qt could be for example ~/Qt/ where the qt installer placed"
    echo " the binaries and sources under)"
    exit 1
fi

target_path=$1/3rdparty-licenses
qt_path=$2
qt_version=$3

mkdir -p $target_path
cp -a `dirname $0`/../LICENSE.md $target_path

cat > about.hbs <<EOT
<!DOCTYPE html>
<html>
<head>
    <style>
        @media (prefers-color-scheme: dark) {
            body { background: #333; color: white; }
            a { color: skyblue; }
        }
        .container { font-family: sans-serif; max-width: 800px; margin: 0 auto; }
        .intro { text-align: center; }
        .licenses-list { list-style-type: none; margin: 0; padding: 0; }
        .license-used-by { margin-top: -10px; }
        .license-text { max-height: 200px; overflow-y: scroll; white-space: pre-wrap; }
    </style>
</head>
<body>
    <main class="container">
        <div class="intro">
            <p>This program is distributed under the terms outlined in <a href="LICENSE.md">LICENSE.md</a></p>.
            <h1>Third Party Licenses</h1>
            <p>This page lists the licenses of the dependencies used by this program.</p>
        </div>

        <h2>Overview of licenses:</h2>
        <ul class="licenses-overview">
            {{#each overview}}
            <li><a href="#{{id}}">{{name}}</a> ({{count}})</li>
            {{/each}}
        </ul>

        <h2>All license text:</h2>
        <ul class="licenses-list">
            {{#each licenses}}
            <li class="license">
                <h3 id="{{id}}">{{name}}</h3>
                <h4>Used by:</h4>
                <ul class="license-used-by">
                    {{#each used_by}}
                    <li><a
                            href="{{#if crate.repository}} {{crate.repository}} {{else}} https://crates.io/crates/{{crate.name}} {{/if}}">{{crate.name}}
                            {{crate.version}}</a></li>
                    {{/each}}
                </ul>
                <pre class="license-text">{{text}}</pre>
            </li>
            {{/each}}
        </ul>

        <h2>Qt License attribution</h2>
        <p>This program also uses the Qt library, which is licensed under the
        <a href="LICENSE.QT">LGPL v3</a></p>.
        <p>Qt may include additional third-party components: <a href="QtThirdPartySoftware_Listing.txt">QtThirdPartySoftware_Listing.txt</a></p>
    <main></body></html>
EOT

cat > about.toml << EOT
accepted = [
    "MIT",
    "Apache-2.0",
    "MPL-2.0",
    "Zlib",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "BSL-1.0",
    "ISC",
    "GPL-3.0", # That's only for SixtyFPS
]
targets = [
    "x86_64-unknown-linux-gnu",
    "x86_64-pc-windows-msvc",
    "x86_64-apple-darwin",
]
ignore-build-dependencies = true
ignore-dev-dependencies = true
EOT

cargo about generate about.hbs -o $target_path/index.html

cp sixtyfps_runtime/rendering_backends/LICENSE.QT sixtyfps_runtime/rendering_backends/QtThirdPartySoftware_Listing.txt $target_path/
