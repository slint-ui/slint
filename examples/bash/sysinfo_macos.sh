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

os_name=`sw_vers -productName`
os_version=`sw_vers -productVersion`
os_name="$os_name $os_version"
uptime=`uptime | awk 'BEGIN{FS="up |,"}{print $2}'`
cpu_count=`sysctl -n hw.ncpu`
cpu_vendor=`sysctl -n machdep.cpu.vendor`
cpu_model=`sysctl -n machdep.cpu.brand_string`
mem_size_kb=`sysctl -n hw.memsize`
partitions=`df -lk | tail -n+2 | sed 's/\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)/{ "dev": "\1", "mnt": "\9", "total": \2, "used": \3 },/' | sed '$s/,$//'`

sixtyfps-viewer `dirname $0`/sysinfo.60 --load-data - <<EOT
{
    "os_name": "$os_name",
    "uptime": "$uptime",
    "cpu_count": "$cpu_count",
    "cpu_vendor": "$cpu_vendor",
    "cpu_model": "$cpu_model",
    "mem_size_kb": $mem_size_kb,
    "partitions": [ $partitions ]
}
EOT
