// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Diagnostics.CodeAnalysis;
using Slint.Interop;

namespace Slint;

/// <summary>Compiles <c>.slint</c> files and sources into a <see cref="CompilationResult"/>.</summary>
public sealed unsafe class Compiler : IDisposable
{
    private readonly CompilerHandle _handle;
    private string? _style;
    private string? _translationDomain;
    private IReadOnlyList<string> _includePaths = [];
    private IReadOnlyDictionary<string, string> _libraryPaths = new Dictionary<string, string>();

    /// <summary>Creates a compiler with the default configuration.</summary>
    public Compiler()
    {
        _handle = NativeMethods.slint_dotnet_compiler_new();
        NativeError.Check(_handle);
    }

    /// <summary>The widget style, such as <c>fluent</c> or <c>cupertino</c>; <c>null</c> until set, which selects the default.</summary>
    [DisallowNull]
    public string? Style
    {
        get => _style;
        set
        {
            using var style = new PinnedUtf8(value);
            NativeError.Check(NativeMethods.slint_dotnet_compiler_set_style(_handle, style.Str));
            _style = value;
        }
    }

    /// <summary>The gettext domain for <c>@tr()</c> translations.</summary>
    [DisallowNull]
    public string? TranslationDomain
    {
        get => _translationDomain;
        set
        {
            using var domain = new PinnedUtf8(value);
            NativeError.Check(NativeMethods.slint_dotnet_compiler_set_translation_domain(_handle, domain.Str));
            _translationDomain = value;
        }
    }

    /// <summary>The directories that <c>import</c> statements search.</summary>
    public IReadOnlyList<string> IncludePaths
    {
        get => _includePaths;
        set
        {
            var paths = value.ToArray();
            using var pinned = new PinnedUtf8Array(paths);
            fixed (Utf8Str* ptr = pinned.Strs)
            {
                NativeError.Check(NativeMethods.slint_dotnet_compiler_set_include_paths(_handle, ptr, (nuint)paths.Length));
            }
            _includePaths = paths;
        }
    }

    /// <summary>Maps library names, as in <c>import { X } from "@name"</c>, to their directory or file.</summary>
    public IReadOnlyDictionary<string, string> LibraryPaths
    {
        get => _libraryPaths;
        set
        {
            var libraries = value.ToDictionary();
            using var names = new PinnedUtf8Array(libraries.Keys);
            using var paths = new PinnedUtf8Array(libraries.Values);
            fixed (Utf8Str* namesPtr = names.Strs)
            fixed (Utf8Str* pathsPtr = paths.Strs)
            {
                NativeError.Check(NativeMethods.slint_dotnet_compiler_set_library_paths(
                    _handle, namesPtr, pathsPtr, (nuint)libraries.Count));
            }
            _libraryPaths = libraries;
        }
    }

    /// <summary>Compiles the <c>.slint</c> file at <paramref name="path"/>.</summary>
    public CompilationResult BuildFromPath(string path)
    {
        using var pathStr = new PinnedUtf8(Path.GetFullPath(path));
        var result = NativeMethods.slint_dotnet_compiler_build_from_path(_handle, pathStr.Str);
        NativeError.Check(result);
        return new CompilationResult(result);
    }

    /// <summary>
    /// Compiles <paramref name="source"/>.
    /// <paramref name="path"/> is used in diagnostics and to resolve relative imports.
    /// </summary>
    public CompilationResult BuildFromSource(string source, string path = "")
    {
        using var sourceStr = new PinnedUtf8(source);
        using var pathStr = new PinnedUtf8(path);
        var result = NativeMethods.slint_dotnet_compiler_build_from_source(_handle, sourceStr.Str, pathStr.Str);
        NativeError.Check(result);
        return new CompilationResult(result);
    }

    /// <summary>Releases the native compiler.</summary>
    public void Dispose() => _handle.Dispose();
}

/// <summary>The severity of a <see cref="Diagnostic"/>.</summary>
public enum DiagnosticLevel
{
    /// <summary>The compilation failed.</summary>
    Error,
    /// <summary>A problem that doesn't stop the compilation.</summary>
    Warning,
    /// <summary>Additional information.</summary>
    Info,
    /// <summary>A note attached to another diagnostic.</summary>
    Note,
}

/// <summary>A message from the compiler, with its location in the source.</summary>
/// <param name="Level">The severity.</param>
/// <param name="Message">The message.</param>
/// <param name="SourceFile">The file the message is about, if any.</param>
/// <param name="Line">The 1-based line, or 0 if there's no location.</param>
/// <param name="Column">The 1-based column, or 0 if there's no location.</param>
public sealed record Diagnostic(DiagnosticLevel Level, string Message, string? SourceFile, int Line, int Column)
{
    /// <inheritdoc/>
    public override string ToString()
    {
        var level = Level.ToString().ToLowerInvariant();
        return SourceFile is null ? $"{level}: {Message}" : $"{SourceFile}:{Line}:{Column}: {level}: {Message}";
    }
}

/// <summary>The components and diagnostics of a compilation.</summary>
public sealed unsafe class CompilationResult : IDisposable
{
    private readonly CompilationResultHandle _handle;

    internal CompilationResult(CompilationResultHandle handle)
    {
        _handle = handle;
        var count = (int)NativeMethods.slint_dotnet_compilation_result_diagnostics_len(handle);
        var diagnostics = new List<Diagnostic>(count);
        for (var i = 0; i < count; i++)
        {
            DiagnosticInfo info;
            NativeError.Check(NativeMethods.slint_dotnet_compilation_result_diagnostic(handle, (nuint)i, &info));
            diagnostics.Add(new Diagnostic(
                (DiagnosticLevel)info.Level,
                info.Message.Take() ?? "",
                info.SourceFile.Take(),
                (int)info.Line,
                (int)info.Column));
        }
        Diagnostics = diagnostics;
        ComponentNames = NameList.TakeNames(NativeMethods.slint_dotnet_compilation_result_component_names(handle)) ?? [];
    }

    /// <summary>The errors, warnings, and notes of the compilation.</summary>
    public IReadOnlyList<Diagnostic> Diagnostics { get; }

    /// <summary>Whether <see cref="Diagnostics"/> contains an error.</summary>
    public bool HasErrors => Diagnostics.Any(d => d.Level == DiagnosticLevel.Error);

    /// <summary>The names of the exported components.</summary>
    public IReadOnlyList<string> ComponentNames { get; }

    /// <summary>Returns the exported component <paramref name="name"/>, or <c>null</c> if there's none.</summary>
    public ComponentDefinition? Component(string name)
    {
        using var nameStr = new PinnedUtf8(name);
        var definition = NativeMethods.slint_dotnet_compilation_result_component(_handle, nameStr.Str);
        return NativeError.Check(definition) ? new ComponentDefinition(definition) : null;
    }

    /// <summary>Releases the native result; components already returned stay valid.</summary>
    public void Dispose() => _handle.Dispose();
}
