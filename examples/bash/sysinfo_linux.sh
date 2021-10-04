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

uptime=`uptime -p | cut -d " " -f2-`
cpu_count=`grep processor /proc/cpuinfo | wc -l`
cpu_vendor=`awk -F ": " '/vendor_id/{ print $2; exit}' < /proc/cpuinfo`
cpu_model=`awk -F ": " '/model name/{ print $2; exit}' < /proc/cpuinfo`
mem_size_kb=`sed -n -e "s,MemTotal:\s\+\(.*\)\s\+.\+,\1,p"< /proc/meminfo`
partitions=`df --block-size=1 | tail -n+2 | sed 's/\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)/{ "dev": "\1", "mnt": "\6", "total": \2, "used": \3 },/' | sed '$s/,$//'`

sixtyfps-viewer `dirname $0`/sysinfo.60 --load-data - <<EOT
{
    "uptime": "$uptime",
    "cpu_count": "$cpu_count",
    "cpu_vendor": "$cpu_vendor",
    "cpu_model": "$cpu_model",
    "mem_size_kb": $mem_size_kb,
    "partitions": [ $partitions ]
}
EOT
