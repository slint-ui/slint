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

cd `dirname $0`/../vscode_extension

git checkout README.md
git checkout package.json

# The version number is a shorterned time stamp of the last commit
nightly_version=`git log -1 --format=%cd --date="format:%Y.%-m.%-d%H"`
last_commit=`git log -1 --format=%H`

# Prepare a modified package.json that has the generated version
# and nightly in the name

git show HEAD:./package.json | jq --arg nightly_version "${nightly_version}" '
.version = $nightly_version |
.name += "-nightly" |
.displayName += " (Nightly)" |
.description += " (Nightly)"' > package.json

cat >README.md <<EOT
# SixtyFPS for Visual Studio Code Nightly

*Note: This is the nightly preview version of the VS Code extension.*

It is published a regular intervals using the latest development code, to
preview new features and test bug fixes. This means that it can be broken
or unstable.
EOT
git show HEAD:./README.md | sed '/^# SixtyFPS for Visual Studio Code$/d;/^## Building from Source$/,$d' >> README.md

cat > CHANGELOG.md <<EOT
This nightly build was created from commit $last_commit
EOT

echo "package.json, REAMDE.md, etc. have been modified. You can package the extension now. Run git checkout afterwards to undo the modifications done by this script."