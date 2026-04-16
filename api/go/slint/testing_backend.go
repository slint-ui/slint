// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//go:build slint_testing
// +build slint_testing

package slint

/*
#cgo CFLAGS: -I${SRCDIR}
#include "bridge.h"
*/
import "C"

func InitTestingBackend() {
	C.slint_testing_init_backend()
}

func MockElapsedTime(timeInMs uint64) {
	C.slint_testing_mock_elapsed_time(C.uint64_t(timeInMs))
}
