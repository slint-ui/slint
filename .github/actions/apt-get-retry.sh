#!/bin/bash
# Copyright © Klarälvdalens Datakonsult AB, a KDAB Group company, info@kdab.com, author David Faure <david.faure@kdab.com>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

# Usage: apt-get-retry.sh update
#        apt-get-retry.sh install [apt-get options] <packages>
#
# Runs apt-get as root, and retries the downloads with a short timeout:
# a slow mirror connection can trickle data for many minutes.
# apt resumes from what earlier attempts downloaded.

set -u

sudo=""
if [ "$(id -u)" -ne 0 ]; then
    sudo=sudo
fi

download() {
    for attempt in 1 2 3; do
        $sudo timeout -k 10 200 apt-get "$@" && return 0
        if [ $? -eq 124 ]; then
            echo "apt-get $1 timed out (attempt $attempt of 3)"
        else
            echo "apt-get $1 failed (attempt $attempt of 3)"
        fi
    done
    return 1
}

case "${1:-}" in
update)
    # apt-get update only warns about an index it failed to fetch.
    # The last attempt accepts that, for a broken third-party repository,
    # or a mirror that stayed slow: the install then fails on what's missing.
    download "$@" --error-on=any || $sudo timeout -k 10 200 apt-get "$@"
    ;;
install)
    # Only the download is retried: killing dpkg halfway leaves a broken install.
    download "$@" --download-only || exit 1
    # This timeout only catches a hang, such as a stuck dpkg lock.
    $sudo timeout -k 10 600 apt-get "$@"
    ;;
*)
    echo "apt-get-retry.sh: unsupported command '${1:-}'" >&2
    exit 2
    ;;
esac
