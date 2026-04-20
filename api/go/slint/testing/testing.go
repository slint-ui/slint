// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//go:build slint_testing
// +build slint_testing

package slint_testing

import slint "github.com/slint-ui/slint/api/go/slint"

func InitTestingBackend() {
	slint.InitTestingBackend()
}

func MockElapsedTime(timeInMs uint64) {
	slint.MockElapsedTime(timeInMs)
}

type componentWithInner interface {
	Inner() *slint.ComponentInstance
}

func SendMouseClick(component componentWithInner, x float64, y float64) {
	component.Inner().SendMouseClick(x, y)
}
