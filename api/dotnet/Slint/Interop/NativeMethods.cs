// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;

namespace Slint.Interop;

/// <summary>The exports of <c>api/dotnet/native</c>.</summary>
internal static unsafe partial class NativeMethods
{
    public const string Library = "slint_dotnet";

    // lib.rs

    [LibraryImport(Library)]
    public static partial void slint_dotnet_string_free(OwnedString s);

    [LibraryImport(Library)]
    public static partial nuint slint_dotnet_name_list_len(nint list);

    [LibraryImport(Library)]
    public static partial Utf8Str slint_dotnet_name_list_get(nint list, nuint index, int* tag);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_name_list_free(nint list);

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_last_error();

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_run_event_loop();

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_quit_event_loop();

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_invoke_from_event_loop(
        delegate* unmanaged<nint, void> callback,
        nint userData);

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_build_features();

    // value.rs

    [LibraryImport(Library)]
    public static partial int slint_dotnet_value_kind(nint value);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_value_free(nint value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_clone(nint value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_void();

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_number(double number);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_bool([MarshalAs(UnmanagedType.U1)] bool b);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_string(Utf8Str s);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_color(uint argb);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_enum(Utf8Str enumName, Utf8Str value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_struct();

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_model(nint* values, nuint count);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_value_to_number(nint value, double* result);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_value_to_bool(nint value, bool* result);

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_value_to_string(nint value);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_value_to_color(nint value, uint* argb);

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_value_enum_name(nint value);

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_value_enum_value(nint value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_model_row_count(nint value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_model_row_data(nint value, nuint row);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_struct_field_names(nint value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_struct_get_field(nint value, Utf8Str name);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_value_struct_set_field(nint value, Utf8Str name, nint field);

    // interpreter.rs

    [LibraryImport(Library)]
    public static partial CompilerHandle slint_dotnet_compiler_new();

    [LibraryImport(Library)]
    public static partial void slint_dotnet_compiler_free(nint compiler);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_compiler_set_style(CompilerHandle compiler, Utf8Str style);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_compiler_set_translation_domain(CompilerHandle compiler, Utf8Str domain);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_compiler_set_include_paths(
        CompilerHandle compiler,
        Utf8Str* paths,
        nuint count);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_compiler_set_library_paths(
        CompilerHandle compiler,
        Utf8Str* names,
        Utf8Str* paths,
        nuint count);

    [LibraryImport(Library)]
    public static partial CompilationResultHandle slint_dotnet_compiler_build_from_path(
        CompilerHandle compiler,
        Utf8Str path);

    [LibraryImport(Library)]
    public static partial CompilationResultHandle slint_dotnet_compiler_build_from_source(
        CompilerHandle compiler,
        Utf8Str source,
        Utf8Str path);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_compilation_result_free(nint result);

    [LibraryImport(Library)]
    public static partial nuint slint_dotnet_compilation_result_diagnostics_len(CompilationResultHandle result);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_compilation_result_diagnostic(
        CompilationResultHandle result,
        nuint index,
        DiagnosticInfo* info);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_compilation_result_component_names(CompilationResultHandle result);

    [LibraryImport(Library)]
    public static partial ComponentDefinitionHandle slint_dotnet_compilation_result_component(
        CompilationResultHandle result,
        Utf8Str name);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_component_definition_free(nint definition);

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_component_definition_name(ComponentDefinitionHandle definition);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_definition_properties(ComponentDefinitionHandle definition);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_definition_callbacks(ComponentDefinitionHandle definition);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_definition_functions(ComponentDefinitionHandle definition);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_definition_globals(ComponentDefinitionHandle definition);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_definition_global_properties(
        ComponentDefinitionHandle definition,
        Utf8Str global);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_definition_global_callbacks(
        ComponentDefinitionHandle definition,
        Utf8Str global);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_definition_global_functions(
        ComponentDefinitionHandle definition,
        Utf8Str global);

    [LibraryImport(Library)]
    public static partial ComponentInstanceHandle slint_dotnet_component_definition_create(
        ComponentDefinitionHandle definition);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_component_instance_free(nint instance);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_show(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_hide(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_run(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_instance_get_property(
        ComponentInstanceHandle instance,
        Utf8Str name);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_set_property(
        ComponentInstanceHandle instance,
        Utf8Str name,
        nint value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_instance_get_global_property(
        ComponentInstanceHandle instance,
        Utf8Str global,
        Utf8Str name);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_set_global_property(
        ComponentInstanceHandle instance,
        Utf8Str global,
        Utf8Str name,
        nint value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_instance_invoke(
        ComponentInstanceHandle instance,
        Utf8Str name,
        nint* args,
        nuint count);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_component_instance_invoke_global(
        ComponentInstanceHandle instance,
        Utf8Str global,
        Utf8Str name,
        nint* args,
        nuint count);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_set_callback(
        ComponentInstanceHandle instance,
        Utf8Str name,
        delegate* unmanaged<nint, nint*, nuint, nint*, void> callback,
        nint userData,
        delegate* unmanaged<nint, void> dropUserData);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_set_global_callback(
        ComponentInstanceHandle instance,
        Utf8Str global,
        Utf8Str name,
        delegate* unmanaged<nint, nint*, nuint, nint*, void> callback,
        nint userData,
        delegate* unmanaged<nint, void> dropUserData);
}
