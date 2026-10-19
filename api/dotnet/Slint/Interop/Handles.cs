// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;

namespace Slint.Interop;

internal abstract class NativeHandle() : SafeHandle(IntPtr.Zero, ownsHandle: true)
{
    public override bool IsInvalid => handle == IntPtr.Zero;
}

internal sealed class CompilerHandle : NativeHandle
{
    protected override bool ReleaseHandle()
    {
        NativeMethods.slint_dotnet_compiler_free(handle);
        return true;
    }
}

internal sealed class CompilationResultHandle : NativeHandle
{
    protected override bool ReleaseHandle()
    {
        NativeMethods.slint_dotnet_compilation_result_free(handle);
        return true;
    }
}

internal sealed class ComponentDefinitionHandle : NativeHandle
{
    protected override bool ReleaseHandle()
    {
        NativeMethods.slint_dotnet_component_definition_free(handle);
        return true;
    }
}

internal sealed class ComponentInstanceHandle : NativeHandle
{
    protected override bool ReleaseHandle()
    {
        NativeMethods.slint_dotnet_component_instance_free(handle);
        return true;
    }
}

internal sealed class ValueHandle : NativeHandle
{
    public ValueHandle()
    {
    }

    public ValueHandle(nint value)
    {
        SetHandle(value);
    }

    protected override bool ReleaseHandle()
    {
        NativeMethods.slint_dotnet_value_free(handle);
        return true;
    }
}

/// <summary>Reads and frees a native <c>NameList</c>.</summary>
internal static unsafe class NameList
{
    public static List<(string Name, int Tag)>? Take(nint list)
    {
        if (list == 0)
        {
            return null;
        }
        try
        {
            var len = (int)NativeMethods.slint_dotnet_name_list_len(list);
            var result = new List<(string, int)>(len);
            for (var i = 0; i < len; i++)
            {
                int tag;
                var name = NativeMethods.slint_dotnet_name_list_get(list, (nuint)i, &tag);
                result.Add((name.ToString(), tag));
            }
            return result;
        }
        finally
        {
            NativeMethods.slint_dotnet_name_list_free(list);
        }
    }

    public static List<string>? TakeNames(nint list) => Take(list)?.Select(e => e.Name).ToList();
}
