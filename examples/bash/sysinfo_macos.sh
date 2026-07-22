#!/bin/bash -e
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: MIT

os_name=`sw_vers -productName`
os_version=`sw_vers -productVersion`
os_name="$os_name $os_version"
uptime=`uptime | awk 'BEGIN{FS="up |,"}{print $2}'`
cpu_count=`sysctl -n hw.ncpu`
cpu_vendor=`sysctl -n machdep.cpu.vendor`
cpu_model=`sysctl -n machdep.cpu.brand_string`
mem_size_kb=$((`sysctl -n hw.memsize` / 1000))
page_size=`vm_stat | grep "Mach Virtual Memory Statistics" | sed -n -e 's,.*page size of \(.*\) bytes.*,\1,p'`
pages_inactive=`vm_stat|grep "Pages inactive" | sed -e "s,Pages inactive:[[:space:]]*\(.*\)\.,\1,"`
file_backed_pages=`vm_stat|grep "File-backed pages" | sed -e "s,File-backed pages:[[:space:]]*\(.*\)\.,\1,"`
buffer_mem_size_kb=$(((pages_inactive + file_backed_pages) * page_size / 1024))
swap_total_mb=`sysctl -n vm.swapusage | sed -n -e 's,total = \(.*\)\..*M.*used.*,\1,p'`
swap_total_kb=$(($swap_total_mb * 1024))
swap_used_mb=`sysctl -n vm.swapusage | sed -n -e 's,.*used = \(.*\)\..*M.*free.*,\1,p'`
swap_used_kb=$((swap_used_mb * 1024))
swap_free_mb=`sysctl -n vm.swapusage | sed -n -e 's,.*free = \(.*\)\..*M.*$,\1,p'`
swap_free_kb=$((swap_free_mb * 1024))
partitions=`df -lk | tail -n+2 | sed 's/\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*\)  *\([^ ]*.*\)/{ "dev": "\1", "mnt": "\9", "total": \2, "free": \4 },/' | sed '$s/,$//'`

slint-viewer `dirname $0`/sysinfo.slint --load-data - <<EOT
{
    "os_name": "$os_name",
    "uptime": "$uptime",
    "cpu_count": "$cpu_count",
    "cpu_vendor": "$cpu_vendor",
    "cpu_model": "$cpu_model",
    "mem_size_kb": $mem_size_kb,
    "buffer_mem_size_kb": $buffer_mem_size_kb,
    "swap_total_kb": $swap_total_kb,
    "swap_used_kb": $swap_used_kb,
    "swap_free_kb": $swap_free_kb,
    "partitions": [ $partitions ]
}
EOT
