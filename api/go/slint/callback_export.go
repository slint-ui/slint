package slint

/*
#cgo CFLAGS: -I${SRCDIR}
#include "bridge.h"
*/
import "C"

import (
	"sync"
	"unsafe"
)

var callbackRegistry sync.Map

func registerCallback(handler func([]Value) Value) uintptr {
	token := callbackSequence.Add(1)
	callbackRegistry.Store(token, handler)
	return token
}

//export slintGoInvokeCallback
func slintGoInvokeCallback(token C.uintptr_t, args **C.SlintGoValue, argLen C.uintptr_t) *C.SlintGoValue {
	handlerValue, ok := callbackRegistry.Load(uintptr(token))
	if !ok {
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

	result := handlerValue.(func([]Value) Value)(values)
	if result.raw() == nil {
		return C.slint_go_value_new()
	}
	return C.slint_go_value_clone(result.raw())
}
