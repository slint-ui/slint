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
    echo "along with an index.md"
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

cat >$target_path/index.md <<EOT
This program is distributed under the terms outlined in [LICENSE.md](LICENSE.md).

This program also uses the Qt library, which is licensed under the
LGPL v3: [qt/LICENSE.LGPLv3](qt/LICENSE.LGPLv3).

Qt may include additional third-party components:

EOT

mkdir -p $target_path/qt
attribution_files=(
    $qt_path/Docs/Qt-$qt_version/qtcore/*-attribution-*.html
    $qt_path/Docs/Qt-$qt_version/qtgui/*-attribution-*.html
    $qt_path/Docs/Qt-$qt_version/qtwidgets/*-attribution-*.html
    $qt_path/Docs/Qt-$qt_version/qtdbus/*-attribution-*.html
)

cp $qt_path/$qt_version/Src/LICENSE.LGPLv3 $target_path/qt

cat >$target_path/qt/index.html <<EOT
<html><head><title>Qt Licenses Attributions</title></head>
<body><h1>Qt Licenses Attributions</h1>
<p><a href="LICENSE.LGPLv3">LICENSE.LGPLv3<+a></p>
<ul>

EOT

for file in ${attribution_files[@]}; do
    cp $file $target_path/qt/
    title=`sed -n -e "s,<title>\(.*\)</title>,\1,p" < $file`
    link=`basename $file`
    echo "<li><a href=\"$link\">$title</a></li>" >> $target_path/qt/index.html
done

echo "</ul></body></html>" > $target_path/qt/index.html
