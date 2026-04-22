// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

//go:build slint_testing
// +build slint_testing

package slint_testing

/*
#cgo CFLAGS: -I${SRCDIR}/..
#include "../bridge.h"
*/
import "C"

import (
	"runtime"
	"unsafe"

	slint "github.com/slint-ui/slint/api/go/slint"
)

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

var emptyByteSentinel byte

func makeByteSlice(value string) (C.SlintGoByteSlice, []byte) {
	buf := []byte(value)
	if len(buf) == 0 {
		return C.SlintGoByteSlice{
			ptr: (*C.uint8_t)(unsafe.Pointer(&emptyByteSentinel)),
			len: 0,
		}, buf
	}
	return C.SlintGoByteSlice{
		ptr: (*C.uint8_t)(unsafe.Pointer(&buf[0])),
		len: C.uintptr_t(len(buf)),
	}, buf
}

type LogicalSize struct {
	Width  float64
	Height float64
}

type LogicalPosition struct {
	X float64
	Y float64
}

type elementHandle struct {
	ptr *C.SlintGoElementHandle
}

type ElementHandle struct {
	handle *elementHandle
}

func wrapElementHandle(ptr *C.SlintGoElementHandle) *ElementHandle {
	if ptr == nil {
		return nil
	}
	handle := &elementHandle{ptr: ptr}
	runtime.SetFinalizer(handle, func(handle *elementHandle) {
		C.slint_go_element_handle_destructor(handle.ptr)
	})
	return &ElementHandle{handle: handle}
}

func (h *ElementHandle) raw() *C.SlintGoElementHandle {
	if h == nil || h.handle == nil {
		return nil
	}
	return h.handle.ptr
}

func FindByElementId(component componentWithInner, id string) *ElementHandle {
	idSlice, idBuf := makeByteSlice(id)
	ptr := C.slint_go_element_handle_find_by_element_id((*C.SlintGoComponentInstance)(component.Inner().RawPointer()), idSlice)
	runtime.KeepAlive(idBuf)
	return wrapElementHandle(ptr)
}

func (h *ElementHandle) Size() LogicalSize {
	var size C.SlintGoLogicalSize
	if h == nil || h.raw() == nil {
		return LogicalSize{}
	}
	C.slint_go_element_handle_size(h.raw(), &size)
	return LogicalSize{Width: float64(size.width), Height: float64(size.height)}
}

func (h *ElementHandle) AbsolutePosition() LogicalPosition {
	var position C.SlintGoLogicalPosition
	if h == nil || h.raw() == nil {
		return LogicalPosition{}
	}
	C.slint_go_element_handle_absolute_position(h.raw(), &position)
	return LogicalPosition{X: float64(position.x), Y: float64(position.y)}
}
