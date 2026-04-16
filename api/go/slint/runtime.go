// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

package slint

/*
#cgo CFLAGS: -I${SRCDIR}
#include <stdlib.h>
#include "bridge.h"
*/
import "C"

import (
	"errors"
	"fmt"
	"runtime"
	"unsafe"
)

type compilationResultHandle struct {
	ptr *C.SlintGoCompilationResult
}

type componentDefinitionHandle struct {
	ptr *C.SlintGoComponentDefinition
}

type componentInstanceHandle struct {
	ptr *C.SlintGoComponentInstance
}

type valueHandle struct {
	ptr *C.SlintGoValue
}

type CompilationResult struct {
	handle *compilationResultHandle
}

type ComponentDefinition struct {
	handle *componentDefinitionHandle
}

type ComponentInstance struct {
	handle    *componentInstanceHandle
	callbacks map[string]*callbackWrapper
}

func (i *ComponentInstance) release() {
	for _, w := range i.callbacks {
		w.release()
	}
	i.callbacks = nil
}

type Value struct {
	handle *valueHandle
}

type ValueType int8

const (
	ValueTypeVoid   ValueType = 0
	ValueTypeNumber ValueType = 1
	ValueTypeString ValueType = 2
	ValueTypeBool   ValueType = 3
	ValueTypeModel  ValueType = 4
	ValueTypeStruct ValueType = 5
	ValueTypeBrush  ValueType = 6
	ValueTypeImage  ValueType = 7
	ValueTypeOther  ValueType = -1
)

var emptyByteSentinel byte
var emptyValueSentinel *C.SlintGoValue

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

func makeValueSlice(values []Value) (C.SlintGoValueSlice, []*C.SlintGoValue) {
	if len(values) == 0 {
		return C.SlintGoValueSlice{
			ptr: (**C.SlintGoValue)(unsafe.Pointer(&emptyValueSentinel)),
			len: 0,
		}, nil
	}
	raw := make([]*C.SlintGoValue, len(values))
	for i, value := range values {
		raw[i] = value.raw()
	}
	return C.SlintGoValueSlice{
		ptr: (**C.SlintGoValue)(unsafe.Pointer(&raw[0])),
		len: C.uintptr_t(len(raw)),
	}, raw
}

func takeCString(ptr *C.char) string {
	if ptr == nil {
		return ""
	}
	defer C.slint_go_string_free(ptr)
	return C.GoString(ptr)
}

func wrapCompilationResult(ptr *C.SlintGoCompilationResult) *CompilationResult {
	if ptr == nil {
		return nil
	}
	handle := &compilationResultHandle{ptr: ptr}
	runtime.SetFinalizer(handle, func(handle *compilationResultHandle) {
		C.slint_go_compilation_result_destructor(handle.ptr)
	})
	return &CompilationResult{handle: handle}
}

func wrapComponentDefinition(ptr *C.SlintGoComponentDefinition) *ComponentDefinition {
	if ptr == nil {
		return nil
	}
	handle := &componentDefinitionHandle{ptr: ptr}
	runtime.SetFinalizer(handle, func(handle *componentDefinitionHandle) {
		C.slint_go_component_definition_destructor(handle.ptr)
	})
	return &ComponentDefinition{handle: handle}
}

func wrapComponentInstance(ptr *C.SlintGoComponentInstance) *ComponentInstance {
	if ptr == nil {
		return nil
	}
	instance := &ComponentInstance{
		handle:    &componentInstanceHandle{ptr: ptr},
		callbacks: map[string]*callbackWrapper{},
	}
	runtime.SetFinalizer(instance, func(instance *ComponentInstance) {
		instance.release()
		C.slint_go_component_instance_destructor(instance.raw())
	})
	return instance
}

func wrapValue(ptr *C.SlintGoValue) Value {
	if ptr == nil {
		return Value{}
	}
	handle := &valueHandle{ptr: ptr}
	runtime.SetFinalizer(handle, func(handle *valueHandle) {
		C.slint_go_value_destructor(handle.ptr)
	})
	return Value{handle: handle}
}

func (r *CompilationResult) raw() *C.SlintGoCompilationResult {
	if r == nil || r.handle == nil {
		return nil
	}
	return r.handle.ptr
}

func (d *ComponentDefinition) raw() *C.SlintGoComponentDefinition {
	if d == nil || d.handle == nil {
		return nil
	}
	return d.handle.ptr
}

func (i *ComponentInstance) raw() *C.SlintGoComponentInstance {
	if i == nil || i.handle == nil {
		return nil
	}
	return i.handle.ptr
}

func (v Value) raw() *C.SlintGoValue {
	if v.handle == nil {
		return nil
	}
	return v.handle.ptr
}

func (v Value) rawOrVoid() *C.SlintGoValue {
	if ptr := v.raw(); ptr != nil {
		return ptr
	}
	return C.slint_go_value_new()
}

func CompileSource(path string, source string) (*CompilationResult, error) {
	sourceSlice, sourceBuf := makeByteSlice(source)
	pathSlice, pathBuf := makeByteSlice(path)
	ptr := C.slint_go_compile_source(sourceSlice, pathSlice)
	runtime.KeepAlive(sourceBuf)
	runtime.KeepAlive(pathBuf)
	result := wrapCompilationResult(ptr)
	if result == nil {
		return nil, errors.New("slint: compilation failed")
	}
	if C.slint_go_compilation_result_has_errors(result.raw()) {
		return nil, fmt.Errorf("slint compilation failed:\n%s", result.Diagnostics())
	}
	return result, nil
}

func CompilePath(path string) (*CompilationResult, error) {
	pathSlice, pathBuf := makeByteSlice(path)
	ptr := C.slint_go_compile_path(pathSlice)
	runtime.KeepAlive(pathBuf)
	result := wrapCompilationResult(ptr)
	if result == nil {
		return nil, errors.New("slint: compilation failed")
	}
	if C.slint_go_compilation_result_has_errors(result.raw()) {
		return nil, fmt.Errorf("slint compilation failed:\n%s", result.Diagnostics())
	}
	return result, nil
}

func (r *CompilationResult) Diagnostics() string {
	return takeCString(C.slint_go_compilation_result_diagnostics(r.raw()))
}

func (r *CompilationResult) Component(name string) *ComponentDefinition {
	nameSlice, nameBuf := makeByteSlice(name)
	definition := wrapComponentDefinition(C.slint_go_compilation_result_component(r.raw(), nameSlice))
	runtime.KeepAlive(nameBuf)
	return definition
}

func (d *ComponentDefinition) Create() (*ComponentInstance, error) {
	var errorMessage *C.char
	instance := wrapComponentInstance(C.slint_go_component_definition_create(d.raw(), &errorMessage))
	if instance == nil {
		if errorMessage != nil {
			return nil, fmt.Errorf("slint: failed to create component instance: %s", takeCString(errorMessage))
		}
		return nil, errors.New("slint: failed to create component instance")
	}
	return instance, nil
}

func (i *ComponentInstance) Show() error {
	if !bool(C.slint_go_component_instance_show(i.raw())) {
		return errors.New("slint: failed to show component")
	}
	return nil
}

func (i *ComponentInstance) Hide() error {
	if !bool(C.slint_go_component_instance_hide(i.raw())) {
		return errors.New("slint: failed to hide component")
	}
	return nil
}

func (i *ComponentInstance) Run() error {
	if !bool(C.slint_go_component_instance_run(i.raw())) {
		return errors.New("slint: failed to run component")
	}
	return nil
}

func (i *ComponentInstance) GetProperty(name string) (Value, error) {
	nameSlice, nameBuf := makeByteSlice(name)
	value := wrapValue(C.slint_go_component_instance_get_property(i.raw(), nameSlice))
	runtime.KeepAlive(nameBuf)
	if value.raw() == nil {
		return VoidValue(), fmt.Errorf("slint: no such property %q", name)
	}
	return value, nil
}

func (i *ComponentInstance) SetProperty(name string, value Value) error {
	nameSlice, nameBuf := makeByteSlice(name)
	rawValue := value.rawOrVoid()
	if value.raw() == nil {
		defer C.slint_go_value_destructor(rawValue)
	}
	ok := C.slint_go_component_instance_set_property(i.raw(), nameSlice, rawValue)
	runtime.KeepAlive(nameBuf)
	if !bool(ok) {
		return fmt.Errorf("slint: failed to set property %q", name)
	}
	return nil
}

func (i *ComponentInstance) Invoke(name string, args ...Value) (Value, error) {
	nameSlice, nameBuf := makeByteSlice(name)
	argSlice, rawArgs := makeValueSlice(args)
	value := wrapValue(C.slint_go_component_instance_invoke(i.raw(), nameSlice, argSlice))
	runtime.KeepAlive(nameBuf)
	runtime.KeepAlive(rawArgs)
	if value.raw() == nil {
		return VoidValue(), fmt.Errorf("slint: failed to invoke %q", name)
	}
	return value, nil
}

func (i *ComponentInstance) GetGlobalProperty(global string, property string) (Value, error) {
	globalSlice, globalBuf := makeByteSlice(global)
	propertySlice, propertyBuf := makeByteSlice(property)
	value := wrapValue(C.slint_go_component_instance_get_global_property(i.raw(), globalSlice, propertySlice))
	runtime.KeepAlive(globalBuf)
	runtime.KeepAlive(propertyBuf)
	if value.raw() == nil {
		return VoidValue(), fmt.Errorf("slint: no such global property %q.%q", global, property)
	}
	return value, nil
}

func (i *ComponentInstance) SetGlobalProperty(global string, property string, value Value) error {
	globalSlice, globalBuf := makeByteSlice(global)
	propertySlice, propertyBuf := makeByteSlice(property)
	rawValue := value.rawOrVoid()
	if value.raw() == nil {
		defer C.slint_go_value_destructor(rawValue)
	}
	ok := C.slint_go_component_instance_set_global_property(i.raw(), globalSlice, propertySlice, rawValue)
	runtime.KeepAlive(globalBuf)
	runtime.KeepAlive(propertyBuf)
	if !bool(ok) {
		return fmt.Errorf("slint: failed to set global property %q.%q", global, property)
	}
	return nil
}

func (i *ComponentInstance) InvokeGlobal(global string, callable string, args ...Value) (Value, error) {
	globalSlice, globalBuf := makeByteSlice(global)
	callableSlice, callableBuf := makeByteSlice(callable)
	argSlice, rawArgs := makeValueSlice(args)
	value := wrapValue(C.slint_go_component_instance_invoke_global(i.raw(), globalSlice, callableSlice, argSlice))
	runtime.KeepAlive(globalBuf)
	runtime.KeepAlive(callableBuf)
	runtime.KeepAlive(rawArgs)
	if value.raw() == nil {
		return VoidValue(), fmt.Errorf("slint: failed to invoke global callable %q.%q", global, callable)
	}
	return value, nil
}

func (i *ComponentInstance) SetCallback(name string, handler func([]Value) Value) error {
	wrapper := newCallbackWrapper(handler)
	nameSlice, nameBuf := makeByteSlice(name)
	ok := C.slint_go_component_instance_set_callback(
		i.raw(),
		nameSlice,
		// Not pretty, but we use uintptr_t instead of passing unsafe.Pointer directly.
		// Otherwise, a panic occurs because the objects inside callbackWrapper are not pinned. (runtime error: argument of cgo function has Go pointer to unpinned Go pointer)
		// This is fine because cgo does not access any of the fields of that struct.
		C.uintptr_t(uintptr(unsafe.Pointer(wrapper))),
		(C.SlintGoCallback)(C.slintGoInvokeCallback),
	)
	runtime.KeepAlive(nameBuf)
	if !bool(ok) {
		return fmt.Errorf("slint: failed to set callback %q", name)
	}
	callbackKey := "local:" + name
	if prev, has := i.callbacks[callbackKey]; has {
		prev.release()
	}
	i.callbacks[callbackKey] = wrapper

	return nil
}

func (i *ComponentInstance) SetLocalCallback(name string, handler func([]Value) Value) error {
	wrapper := newCallbackWrapper(handler)
	nameSlice, nameBuf := makeByteSlice(name)
	ok := C.slint_go_component_instance_set_callback(
		i.raw(),
		nameSlice,
		C.uintptr_t(uintptr(unsafe.Pointer(wrapper))),
		(C.SlintGoCallback)(C.slintGoInvokeCallback),
	)
	runtime.KeepAlive(nameBuf)
	if !bool(ok) {
		return fmt.Errorf("slint: failed to set callback %q", name)
	}

	callbackKey := "local:" + name
	if prev, has := i.callbacks[callbackKey]; has {
		prev.release()
	}
	i.callbacks[callbackKey] = wrapper

	return nil
}

func (i *ComponentInstance) SetGlobalCallback(global string, name string, handler func([]Value) Value) error {
	wrapper := newCallbackWrapper(handler)
	globalSlice, globalBuf := makeByteSlice(global)
	nameSlice, nameBuf := makeByteSlice(name)
	ok := C.slint_go_component_instance_set_global_callback(
		i.raw(),
		globalSlice,
		nameSlice,
		C.uintptr_t(uintptr(unsafe.Pointer(wrapper))),
		(C.SlintGoCallback)(C.slintGoInvokeCallback),
	)
	runtime.KeepAlive(globalBuf)
	runtime.KeepAlive(nameBuf)
	if !bool(ok) {
		return fmt.Errorf("slint: failed to set global callback %q.%q", global, name)
	}

	callbackKey := "global:" + global + "\x00" + name
	if prev, has := i.callbacks[callbackKey]; has {
		prev.release()
	}
	i.callbacks[callbackKey] = wrapper

	return nil
}

func VoidValue() Value {
	return wrapValue(C.slint_go_value_new())
}

func NumberValue(value float64) Value {
	return wrapValue(C.slint_go_value_new_number(C.double(value)))
}

func BoolValue(value bool) Value {
	return wrapValue(C.slint_go_value_new_bool(C.bool(value)))
}

func EnumValue(enumName string, value string) Value {
	enumNameSlice, enumNameBuf := makeByteSlice(enumName)
	valueSlice, valueBuf := makeByteSlice(value)
	result := wrapValue(C.slint_go_value_new_enumeration_value(enumNameSlice, valueSlice))
	runtime.KeepAlive(enumNameBuf)
	runtime.KeepAlive(valueBuf)
	return result
}

func StringValue(value string) Value {
	valueSlice, valueBuf := makeByteSlice(value)
	result := wrapValue(C.slint_go_value_new_string(valueSlice))
	runtime.KeepAlive(valueBuf)
	return result
}

func (v Value) Clone() Value {
	if v.raw() == nil {
		return VoidValue()
	}
	return wrapValue(C.slint_go_value_clone(v.raw()))
}

func (v Value) Type() ValueType {
	if v.raw() == nil {
		return ValueTypeVoid
	}
	return ValueType(C.slint_go_value_type(v.rawOrVoid()))
}

func (v Value) String() (string, error) {
	if v.raw() == nil {
		return "", errors.New("slint: value is void")
	}
	ptr := C.slint_go_value_to_string(v.rawOrVoid())
	if ptr == nil {
		return "", errors.New("slint: value is not a string")
	}
	return takeCString(ptr), nil
}

func (v Value) Number() (float64, error) {
	if v.raw() == nil {
		return 0, errors.New("slint: value is void")
	}
	var number C.double
	if !bool(C.slint_go_value_to_number(v.rawOrVoid(), &number)) {
		return 0, errors.New("slint: value is not a number")
	}
	return float64(number), nil
}

func (v Value) Bool() (bool, error) {
	if v.raw() == nil {
		return false, errors.New("slint: value is void")
	}
	var value C.bool
	if !bool(C.slint_go_value_to_bool(v.rawOrVoid(), &value)) {
		return false, errors.New("slint: value is not a bool")
	}
	return bool(value), nil
}
