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
    echo "(The path to Qt could be for example ~/Qt/ where the qt installer placed"
    echo " the binaries and sources under)"
    exit 1
fi

target_path=$1/3rdparty-licenses
qt_path=$2
qt_version=$3

mkdir -p $target_path
cp -a `dirname $0`/../LICENSE.md $target_path

mkdir -p $target_path/qt
attribution_files=(
    $qt_path/Docs/Qt-$qt_version/qtcore/*-attribution-*.html
    $qt_path/Docs/Qt-$qt_version/qtgui/*-attribution-*.html
    $qt_path/Docs/Qt-$qt_version/qtwidgets/*-attribution-*.html
    $qt_path/Docs/Qt-$qt_version/qtdbus/*-attribution-*.html
)

cp $qt_path/$qt_version/Src/LICENSE.LGPLv3 $target_path/qt

cat >$target_path/qt/index.md <<EOT
[LICENSE.LGPLv3](LICENSE.LGPLv3)

EOT

for file in ${attribution_files[@]}; do
    cp $file $target_path/qt/
    title=`sed -n -e "s,<title>\(.*\)</title>,\1,p" < $file`
    link=`basename $file`
    echo "[$title]($link)" >> $target_path/qt/index.md
    echo "" >> $target_path/qt/index.md
done
