// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using Slint.Interop;

namespace Slint.Tests;

/// <summary>Checks the declarations in <see cref="NativeMethods"/> against the native library.</summary>
[TestClass]
public sealed class InteropTests
{
    [TestMethod]
    public void StructsHaveTheNativeSize()
    {
        Assert.AreEqual(NativeMethods.slint_dotnet_struct_size(0), (nuint)Unsafe.SizeOf<Utf8Str>());
        Assert.AreEqual(NativeMethods.slint_dotnet_struct_size(1), (nuint)Unsafe.SizeOf<OwnedString>());
        Assert.AreEqual(NativeMethods.slint_dotnet_struct_size(2), (nuint)Unsafe.SizeOf<DiagnosticInfo>());
        Assert.AreEqual(NativeMethods.slint_dotnet_struct_size(3), (nuint)Unsafe.SizeOf<ModelVTable>());
    }

    [TestMethod]
    public void EveryDeclarationHasANativeExport()
    {
        Assert.IsTrue(NativeLibrary.TryLoad(NativeMethods.Library, typeof(NativeMethods).Assembly, null, out var library));
        var declarations = typeof(NativeMethods)
            .GetMethods(BindingFlags.Public | BindingFlags.Static)
            .Where(m => m.GetCustomAttribute<LibraryImportAttribute>() != null)
            .Select(m => m.GetCustomAttribute<LibraryImportAttribute>()!.EntryPoint ?? m.Name)
            .ToList();
        Assert.IsGreaterThan(100, declarations.Count);
        var missing = declarations.Where(name => !NativeLibrary.TryGetExport(library, name, out _)).ToList();
        Assert.IsEmpty(missing, "declared but not exported: " + string.Join(", ", missing));
    }
}
