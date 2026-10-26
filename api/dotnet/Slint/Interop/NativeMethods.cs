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
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_backend_is_winit();

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_build_features();

    [LibraryImport(Library)]
    public static partial nuint slint_dotnet_struct_size(int kind);

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
    public static partial nint slint_dotnet_value_new_image();

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_image_from_path(Utf8Str path);

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_value_image_path(ValueHandle value);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_value_image_size(ValueHandle value, uint* width, uint* height);

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
    public static partial bool slint_dotnet_compiler_set_bundled_translations_path(CompilerHandle compiler, Utf8Str path);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_compiler_set_default_translation_context(
        CompilerHandle compiler,
        [MarshalAs(UnmanagedType.U1)] bool componentName);

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
    public static partial ComponentInstanceHandle slint_dotnet_live_component_new(
        Utf8Str path,
        Utf8Str component,
        Utf8Str style,
        Utf8Str* includePaths,
        nuint includeCount,
        Utf8Str* libraryNames,
        Utf8Str* libraryPaths,
        nuint libraryCount,
        Utf8Str translationDomain,
        Utf8Str bundledTranslationsPath,
        [MarshalAs(UnmanagedType.U1)] bool defaultTranslationContext);

    // translations.rs

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_select_bundled_translation(Utf8Str language);

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_string_new(Utf8Str s);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_set_translator(
        delegate* unmanaged<nint, Utf8Str, Utf8Str, ulong, byte, Utf8Str, byte, OwnedString> translate,
        nint userData,
        delegate* unmanaged<nint, void> dropUserData);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_live_component_set_reload_hook(
        ComponentInstanceHandle instance,
        delegate* unmanaged<nint, void> call,
        nint userData,
        delegate* unmanaged<nint, void> dropUserData);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_show(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_hide(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_component_instance_is_window(ComponentInstanceHandle instance);

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

    // models.rs

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_model_new(ModelVTable vtable, nint userData, nint* weak);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_model_weak_upgrade(ModelWeakHandle weak);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_model_weak_free(nint weak);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_model_notify_row_changed(ModelWeakHandle weak, nuint row);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_model_notify_row_added(ModelWeakHandle weak, nuint index, nuint count);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_model_notify_row_removed(ModelWeakHandle weak, nuint index, nuint count);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_model_notify_reset(ModelWeakHandle weak);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_managed_model(nint value);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_value_model_set_row_data(ValueHandle value, nuint row, nint data);

    // timer.rs

    [LibraryImport(Library)]
    public static partial TimerHandle slint_dotnet_timer_new();

    [LibraryImport(Library)]
    public static partial void slint_dotnet_timer_free(nint timer);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_timer_start(
        TimerHandle timer,
        [MarshalAs(UnmanagedType.U1)] bool repeated,
        ulong intervalMs,
        delegate* unmanaged<nint, void> call,
        nint userData,
        delegate* unmanaged<nint, void> dropUserData);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_timer_single_shot(
        ulong intervalMs,
        delegate* unmanaged<nint, void> call,
        nint userData,
        delegate* unmanaged<nint, void> dropUserData);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_timer_stop(TimerHandle timer);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_timer_restart(TimerHandle timer);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_timer_running(TimerHandle timer);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_timer_set_interval(TimerHandle timer, ulong intervalMs);

    [LibraryImport(Library)]
    public static partial ulong slint_dotnet_timer_interval_ms(TimerHandle timer);

    // brush.rs

    [LibraryImport(Library)]
    public static partial uint slint_dotnet_color_brighter(uint argb, float factor);

    [LibraryImport(Library)]
    public static partial uint slint_dotnet_color_darker(uint argb, float factor);

    [LibraryImport(Library)]
    public static partial uint slint_dotnet_color_transparentize(uint argb, float factor);

    [LibraryImport(Library)]
    public static partial uint slint_dotnet_color_mix(uint argb, uint other, float factor);

    [LibraryImport(Library)]
    public static partial uint slint_dotnet_color_with_alpha(uint argb, float alpha);

    [LibraryImport(Library)]
    public static partial uint slint_dotnet_brush_color(ValueHandle value);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_brush_is_transparent(ValueHandle value);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_brush_is_opaque(ValueHandle value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_brush_derive(ValueHandle value, int operation, float factor);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_value_equals(ValueHandle a, ValueHandle b);

    // special.rs

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_keys(Utf8Str* parts, nuint count);

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_value_describe(ValueHandle value);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_styled_text(int kind, Utf8Str text);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_value_new_data_transfer();

    [LibraryImport(Library)]
    public static partial OwnedString slint_dotnet_data_transfer_plain_text(ValueHandle value);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_data_transfer_set_plain_text(ValueHandle value, Utf8Str text);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_data_transfer_image(ValueHandle value);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_data_transfer_set_image(ValueHandle value, nint image);

    [LibraryImport(Library)]
    public static partial int slint_dotnet_data_transfer_state(ValueHandle value);

    // window.rs

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_window_is_visible(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    public static partial float slint_dotnet_window_scale_factor(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_request_redraw(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_position(ComponentInstanceHandle instance, int* x, int* y);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_set_physical_position(ComponentInstanceHandle instance, int x, int y);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_set_logical_position(ComponentInstanceHandle instance, float x, float y);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_size(ComponentInstanceHandle instance, uint* width, uint* height);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_set_physical_size(ComponentInstanceHandle instance, uint width, uint height);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_set_logical_size(ComponentInstanceHandle instance, float width, float height);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_window_is_fullscreen(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_set_fullscreen(ComponentInstanceHandle instance, [MarshalAs(UnmanagedType.U1)] bool value);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_window_is_maximized(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_set_maximized(ComponentInstanceHandle instance, [MarshalAs(UnmanagedType.U1)] bool value);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_window_is_minimized(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_set_minimized(ComponentInstanceHandle instance, [MarshalAs(UnmanagedType.U1)] bool value);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_on_close_requested(
        ComponentInstanceHandle instance,
        delegate* unmanaged<nint, int> call,
        nint userData,
        delegate* unmanaged<nint, void> dropUserData);

    [LibraryImport(Library)]
    public static partial void slint_dotnet_window_dispatch_close_requested(ComponentInstanceHandle instance);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_testing_init();

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_testing_mock_elapsed_time(ulong ms);

    [LibraryImport(Library)]
    public static partial ulong slint_dotnet_testing_get_mocked_time();

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_testing_send_mouse_click(ComponentInstanceHandle instance, float x, float y);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_testing_send_keyboard_string_sequence(ComponentInstanceHandle instance, Utf8Str sequence);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_testing_send_key_combo(ComponentInstanceHandle instance, Utf8Str* keys, nuint count);

    [LibraryImport(Library)]
    public static partial int slint_dotnet_window_dispatch_event(
        ComponentInstanceHandle instance,
        int kind,
        float x,
        float y,
        float a,
        float b,
        int button,
        [MarshalAs(UnmanagedType.U1)] bool active,
        Utf8Str text);

    [LibraryImport(Library)]
    [return: MarshalAs(UnmanagedType.U1)]
    public static partial bool slint_dotnet_data_transfer_set_user_data(
        ValueHandle value, nint handle, delegate* unmanaged<nint, void> release);

    [LibraryImport(Library)]
    public static partial nint slint_dotnet_data_transfer_user_data(ValueHandle value);
}
