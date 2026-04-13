// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

package slint

/*
#cgo CFLAGS: -I${SRCDIR}
#include "bridge.h"
*/
import "C"

import (
	"runtime"
	"unsafe"
)

type callbackWrapper struct {
	handler func([]Value) Value
	pinner  runtime.Pinner
}

func newCallbackWrapper(handler func([]Value) Value) *callbackWrapper {
	wrapper := &callbackWrapper{handler: handler}
	wrapper.pinner.Pin(wrapper)
	return wrapper
}

func (c *callbackWrapper) release() {
	c.pinner.Unpin()
}

//export slintGoInvokeCallback
func slintGoInvokeCallback(userData unsafe.Pointer, args **C.SlintGoValue, argLen C.uintptr_t) *C.SlintGoValue {
	if userData == nil {
		return C.slint_go_value_new()
	}

	wrapper := (*callbackWrapper)(userData)
	if wrapper == nil || wrapper.handler == nil {
		return C.slint_go_value_new()
	}

	var values []Value
	if args != nil && argLen > 0 {
		rawArgs := unsafe.Slice(args, int(argLen))
		values = make([]Value, len(rawArgs))
		for i, raw := range rawArgs {
			values[i] = wrapValue(C.slint_go_value_clone(raw))
		}
	}

	result := wrapper.handler(values)
	if result.raw() == nil {
		return C.slint_go_value_new()
	}
	return C.slint_go_value_clone(result.raw())
}
