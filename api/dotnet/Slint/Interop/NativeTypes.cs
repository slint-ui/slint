// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
using System.Text;

namespace Slint.Interop;

/// <summary>A borrowed UTF-8 string, see <c>Utf8Str</c> in <c>api/dotnet/native/lib.rs</c>.</summary>
[StructLayout(LayoutKind.Sequential)]
internal readonly unsafe struct Utf8Str(byte* ptr, nuint len)
{
    public readonly byte* Ptr = ptr;
    public readonly nuint Len = len;

    public override string ToString() => Len == 0 ? "" : Encoding.UTF8.GetString(Ptr, checked((int)Len));
}

/// <summary>A string allocated by Rust, see <c>OwnedString</c> in <c>api/dotnet/native/lib.rs</c>.</summary>
[StructLayout(LayoutKind.Sequential)]
internal readonly unsafe struct OwnedString
{
    public readonly byte* Ptr;
    public readonly nuint Len;
    public readonly nuint Cap;

    /// <summary>Copies and frees the string; returns <c>null</c> for no string.</summary>
    public string? Take()
    {
        if (Ptr == null)
        {
            return null;
        }
        try
        {
            return Len == 0 ? "" : Encoding.UTF8.GetString(Ptr, checked((int)Len));
        }
        finally
        {
            NativeMethods.slint_dotnet_string_free(this);
        }
    }
}

[StructLayout(LayoutKind.Sequential)]
internal struct DiagnosticInfo
{
    public int Level;
    public nuint Line;
    public nuint Column;
    public OwnedString Message;
    public OwnedString SourceFile;
}

/// <summary>A UTF-8 copy of a string, pinned until disposed.</summary>
internal unsafe ref struct PinnedUtf8
{
    private GCHandle _handle;

    public PinnedUtf8(string s)
    {
        var bytes = Encoding.UTF8.GetBytes(s);
        _handle = GCHandle.Alloc(bytes, GCHandleType.Pinned);
        Str = new Utf8Str((byte*)_handle.AddrOfPinnedObject(), (nuint)bytes.Length);
    }

    public Utf8Str Str { get; }

    public void Dispose()
    {
        if (_handle.IsAllocated)
        {
            _handle.Free();
        }
    }
}

/// <summary>UTF-8 copies of several strings, pinned until disposed.</summary>
internal sealed unsafe class PinnedUtf8Array : IDisposable
{
    private readonly GCHandle[] _handles;

    public PinnedUtf8Array(IEnumerable<string> strings)
    {
        var buffers = strings.Select(Encoding.UTF8.GetBytes).ToArray();
        _handles = new GCHandle[buffers.Length];
        Strs = new Utf8Str[buffers.Length];
        for (var i = 0; i < buffers.Length; i++)
        {
            _handles[i] = GCHandle.Alloc(buffers[i], GCHandleType.Pinned);
            Strs[i] = new Utf8Str((byte*)_handles[i].AddrOfPinnedObject(), (nuint)buffers[i].Length);
        }
    }

    public Utf8Str[] Strs { get; }

    public void Dispose()
    {
        foreach (var h in _handles)
        {
            h.Free();
        }
    }
}
