// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Reflection;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;

namespace Slint.Interop;

/// <summary>
/// Loads the native library from <c>SLINT_DOTNET_NATIVE_LIBRARY</c> if set,
/// and otherwise leaves it to the default probing,
/// which covers the application directory and <c>runtimes/&lt;rid&gt;/native</c> of a package.
/// </summary>
internal static class NativeLibraryResolver
{
    public const string EnvironmentVariable = "SLINT_DOTNET_NATIVE_LIBRARY";

#pragma warning disable CA2255 // The resolver has to be in place before the first P/Invoke.
    [ModuleInitializer]
#pragma warning restore CA2255
    internal static void Register() =>
        NativeLibrary.SetDllImportResolver(typeof(NativeLibraryResolver).Assembly, Resolve);

    private static nint Resolve(string libraryName, Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (libraryName != NativeMethods.Library)
        {
            return 0;
        }
        if (Environment.GetEnvironmentVariable(EnvironmentVariable) is { Length: > 0 } path)
        {
            return NativeLibrary.Load(path);
        }
        // iOS apps embed the native library as a framework, which the default probing doesn't look into.
        return OperatingSystem.IsIOS() ? NativeLibrary.Load("@rpath/slint_dotnet.framework/slint_dotnet") : 0;
    }
}
