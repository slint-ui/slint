#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

cd `dirname $0`/../editors/vscode

# The version number is a shortened time stamp of the last commit
nightly_version=`git log -1 --format=%cd --date="format:%Y.%-m.%-d%H"`
last_commit=`git log -1 --format=%H`

# Prepare a modified package.json that has the generated version
# and nightly in the name

git show HEAD:./package.json | jq --arg nightly_version "${nightly_version}" '
.version = $nightly_version |
.name += "-nightly" |
.displayName += " (Nightly)" |
.description += " (Nightly)" |
. + {"preview": true}' > package.json

mv README.md README.md.orig

cat >README.md <<EOT
# Slint for Visual Studio Code Nightly

*Note: This is the nightly preview version of the VS Code extension.*

It is published a regular intervals using the latest development code, to
preview new features and test bug fixes. This means that it can be broken
or unstable.
EOT

cat README.md.orig >> README.md
rm README.md.orig

cat > CHANGELOG.md <<EOT
This nightly build was created from commit $last_commit
EOT

echo "package.json, README.md, etc. have been modified. You can package the extension now. Run git checkout afterwards to undo the modifications done by this script."
