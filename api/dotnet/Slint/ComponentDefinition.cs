// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using Slint.Interop;

namespace Slint;

/// <summary>A compiled component; call <see cref="Create"/> to instantiate it.</summary>
public sealed class ComponentDefinition : IDisposable
{
    private readonly ComponentDefinitionHandle _handle;

    internal ComponentDefinition(ComponentDefinitionHandle handle)
    {
        _handle = handle;
    }

    /// <summary>The name of the component.</summary>
    public string Name => NativeMethods.slint_dotnet_component_definition_name(_handle).Take() ?? "";

    /// <summary>The public properties and their types.</summary>
    public IReadOnlyDictionary<string, ValueType> Properties =>
        ToProperties(NativeMethods.slint_dotnet_component_definition_properties(_handle))!;

    /// <summary>The public callbacks.</summary>
    public IReadOnlyList<string> Callbacks =>
        NameList.TakeNames(NativeMethods.slint_dotnet_component_definition_callbacks(_handle))!;

    /// <summary>The public functions.</summary>
    public IReadOnlyList<string> Functions =>
        NameList.TakeNames(NativeMethods.slint_dotnet_component_definition_functions(_handle))!;

    /// <summary>The exported globals.</summary>
    public IReadOnlyList<string> Globals =>
        NameList.TakeNames(NativeMethods.slint_dotnet_component_definition_globals(_handle))!;

    /// <summary>Returns the properties of the global <paramref name="global"/>, or <c>null</c> if there's none.</summary>
    public IReadOnlyDictionary<string, ValueType>? GlobalProperties(string global)
    {
        using var name = new PinnedUtf8(global);
        return ToProperties(CheckList(NativeMethods.slint_dotnet_component_definition_global_properties(_handle, name.Str)));
    }

    /// <summary>Returns the callbacks of the global <paramref name="global"/>, or <c>null</c> if there's none.</summary>
    public IReadOnlyList<string>? GlobalCallbacks(string global)
    {
        using var name = new PinnedUtf8(global);
        return NameList.TakeNames(CheckList(NativeMethods.slint_dotnet_component_definition_global_callbacks(_handle, name.Str)));
    }

    /// <summary>Returns the functions of the global <paramref name="global"/>, or <c>null</c> if there's none.</summary>
    public IReadOnlyList<string>? GlobalFunctions(string global)
    {
        using var name = new PinnedUtf8(global);
        return NameList.TakeNames(CheckList(NativeMethods.slint_dotnet_component_definition_global_functions(_handle, name.Str)));
    }

    /// <summary>Creates a new instance of the component, with its own window.</summary>
    public ComponentInstance Create()
    {
        var instance = NativeMethods.slint_dotnet_component_definition_create(_handle);
        if (!NativeError.Check(instance))
        {
            throw new SlintException($"Failed to create component {Name}");
        }
        return new ComponentInstance(instance);
    }

    /// <summary>Releases the native definition; instances already created stay valid.</summary>
    public void Dispose() => _handle.Dispose();

    private static nint CheckList(nint list)
    {
        if (list == 0 && NativeError.TakeMessage() is { } message)
        {
            throw new SlintException(message);
        }
        return list;
    }

    private static Dictionary<string, ValueType>? ToProperties(nint list) =>
        NameList.Take(list)?.ToDictionary(p => p.Name, p => (ValueType)p.Tag);
}
