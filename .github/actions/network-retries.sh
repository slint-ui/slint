#!/bin/bash
# Copyright © SixtyFPS GmbH <info@slint.dev>
# SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0
# cSpell: ignore curlrc

# Make curl and apt retry transient download failures for the rest of the job.
# The setup-rust and install-linux-dependencies actions run it.

set -eu

# curl reads $CURL_HOME/.curlrc (_curlrc on Windows), also when a build script
# such as skia-bindings' runs it. --retry only retries timeouts and HTTP 408,
# 429, 500, 502, 503 and 504, not a 404.
curl_home="$RUNNER_TEMP/curl-home"
if [ ! -f "$curl_home/.curlrc" ]; then
    mkdir -p "$curl_home"
    printf 'retry = 5\nretry-delay = 10\nretry-connrefused\nconnect-timeout = 30\n' > "$curl_home/.curlrc"
    cp "$curl_home/.curlrc" "$curl_home/_curlrc"
    echo "CURL_HOME=$curl_home" >> "$GITHUB_ENV"
fi

# The Azure Ubuntu mirror sometimes stalls, and apt then waits until the step's
# timeout. Give up on a request sooner and retry it.
if [ "$RUNNER_OS" = "Linux" ] && [ ! -f /etc/apt/apt.conf.d/80-ci-network-retries ]; then
    printf 'Acquire::Retries "5";\nAcquire::http::Timeout "30";\nAcquire::https::Timeout "30";\n' \
        | sudo tee /etc/apt/apt.conf.d/80-ci-network-retries > /dev/null
fi
