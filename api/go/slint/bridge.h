// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

#pragma once

#include <stdbool.h>
#include <stdint.h>

typedef struct SlintGoCompilationResult SlintGoCompilationResult;
typedef struct SlintGoComponentDefinition SlintGoComponentDefinition;
typedef struct SlintGoComponentInstance SlintGoComponentInstance;
typedef struct SlintGoValue SlintGoValue;

typedef struct SlintGoByteSlice
{
    const uint8_t *ptr;
    uintptr_t len;
} SlintGoByteSlice;

typedef struct SlintGoValueSlice
{
    SlintGoValue **ptr;
    uintptr_t len;
} SlintGoValueSlice;

typedef int8_t SlintGoValueType;

typedef SlintGoValue *(*SlintGoCallback)(void *user_data, SlintGoValue **args, uintptr_t arg_len);

void slint_go_string_free(char *value);

SlintGoCompilationResult *slint_go_compile_source(SlintGoByteSlice source, SlintGoByteSlice path);
SlintGoCompilationResult *slint_go_compile_path(SlintGoByteSlice path);
void slint_go_compilation_result_destructor(SlintGoCompilationResult *result);
bool slint_go_compilation_result_has_errors(const SlintGoCompilationResult *result);
char *slint_go_compilation_result_diagnostics(const SlintGoCompilationResult *result);
SlintGoComponentDefinition *
slint_go_compilation_result_component(const SlintGoCompilationResult *result,
                                      SlintGoByteSlice name);

void slint_go_component_definition_destructor(SlintGoComponentDefinition *definition);
SlintGoComponentInstance *
slint_go_component_definition_create(const SlintGoComponentDefinition *definition);

void slint_go_component_instance_destructor(SlintGoComponentInstance *instance);
bool slint_go_component_instance_show(const SlintGoComponentInstance *instance);
bool slint_go_component_instance_hide(const SlintGoComponentInstance *instance);
bool slint_go_component_instance_run(const SlintGoComponentInstance *instance);
SlintGoValue *slint_go_component_instance_get_property(const SlintGoComponentInstance *instance,
                                                       SlintGoByteSlice name);
bool slint_go_component_instance_set_property(const SlintGoComponentInstance *instance,
                                              SlintGoByteSlice name, const SlintGoValue *value);
SlintGoValue *slint_go_component_instance_invoke(const SlintGoComponentInstance *instance,
                                                 SlintGoByteSlice name, SlintGoValueSlice args);
SlintGoValue *
slint_go_component_instance_get_global_property(const SlintGoComponentInstance *instance,
                                                SlintGoByteSlice global, SlintGoByteSlice property);
bool slint_go_component_instance_set_global_property(const SlintGoComponentInstance *instance,
                                                     SlintGoByteSlice global,
                                                     SlintGoByteSlice property,
                                                     const SlintGoValue *value);
SlintGoValue *slint_go_component_instance_invoke_global(const SlintGoComponentInstance *instance,
                                                        SlintGoByteSlice global,
                                                        SlintGoByteSlice callable,
                                                        SlintGoValueSlice args);
bool slint_go_component_instance_set_callback(const SlintGoComponentInstance *instance,
                                              SlintGoByteSlice name, uintptr_t user_data,
                                              SlintGoCallback callback);
bool slint_go_component_instance_set_global_callback(const SlintGoComponentInstance *instance,
                                                     SlintGoByteSlice global, SlintGoByteSlice name,
                                                     uintptr_t user_data, SlintGoCallback callback);

SlintGoValue *slint_go_value_new(void);
SlintGoValue *slint_go_value_clone(const SlintGoValue *value);
void slint_go_value_destructor(SlintGoValue *value);
SlintGoValue *slint_go_value_new_number(double value);
SlintGoValue *slint_go_value_new_string(SlintGoByteSlice value);
SlintGoValue *slint_go_value_new_bool(bool value);
SlintGoValueType slint_go_value_type(const SlintGoValue *value);
char *slint_go_value_to_string(const SlintGoValue *value);
bool slint_go_value_to_number(const SlintGoValue *value, double *out);
bool slint_go_value_to_bool(const SlintGoValue *value, bool *out);
void slint_testing_init_backend(void);

extern SlintGoValue *slintGoInvokeCallback(void *user_data, SlintGoValue **args, uintptr_t arg_len);
