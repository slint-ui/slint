// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using Slint.Interop;

namespace Slint;

/// <summary>The phase of rendering in which a <see cref="RenderingNotifier"/> runs.</summary>
public enum RenderingState
{
    /// <summary>The window and its graphics context exist; create your graphics resources.</summary>
    RenderingSetup,
    /// <summary>Slint is about to render the scene; draw what goes below it.</summary>
    BeforeRendering,
    /// <summary>Slint rendered the scene, before showing it; draw what goes on top of it.</summary>
    AfterRendering,
    /// <summary>The window or its graphics context goes away; release your graphics resources.</summary>
    RenderingTeardown,
}

/// <summary>The kind of graphics API that Slint renders with.</summary>
public enum GraphicsApiKind
{
    /// <summary>An API that the .NET bindings don't give access to, such as Metal or WGPU.</summary>
    Other,
    /// <summary>OpenGL or OpenGL ES; load its functions with <see cref="GraphicsApi.GetProcAddress"/>.</summary>
    NativeOpenGL,
    /// <summary>WebGL in a browser.</summary>
    WebGL,
}

/// <summary>Which row of a borrowed OpenGL texture Slint shows at the top.</summary>
public enum BorrowedOpenGLTextureOrigin
{
    /// <summary>The first row of the texture is at the top.</summary>
    TopLeft,
    /// <summary>The first row of the texture is at the bottom, as OpenGL draws into a texture.</summary>
    BottomLeft,
}

/// <summary>
/// The graphics API that Slint renders with, during a <see cref="RenderingNotifier"/> call.
/// With OpenGL, the window's context is current, so draw with it, and restore the state that you change,
/// such as the bound program, buffers, and textures.
/// </summary>
public readonly unsafe ref struct GraphicsApi
{
    private readonly nint _api;

    internal GraphicsApi(nint api)
    {
        _api = api;
    }

    /// <summary>The kind of graphics API.</summary>
    public GraphicsApiKind Kind => (GraphicsApiKind)NativeMethods.slint_dotnet_graphics_api_kind(_api);

    /// <summary>
    /// The address of the OpenGL function <paramref name="name"/>, such as <c>glDrawArrays</c>, or 0 if there's none.
    /// The address stays valid with the window's context, so load the functions once, in <see cref="RenderingState.RenderingSetup"/>.
    /// </summary>
    /// <exception cref="InvalidOperationException">Slint doesn't render with OpenGL.</exception>
    public nint GetProcAddress(string name)
    {
        ArgumentNullException.ThrowIfNull(name);
        if (Kind != GraphicsApiKind.NativeOpenGL)
        {
            throw new InvalidOperationException($"Slint renders with {Kind}, not OpenGL; require it with BackendSelector.RequireOpenGL");
        }
        using var str = new PinnedUtf8(name);
        var address = NativeMethods.slint_dotnet_graphics_api_get_proc_address(_api, str.Str);
        if (address == 0 && NativeError.TakeMessage() is { } message)
        {
            throw new SlintException(message);
        }
        return address;
    }
}

/// <summary>Runs in each phase of rendering a window, see <see cref="Window.SetRenderingNotifier"/>.</summary>
/// <param name="state">The phase of rendering.</param>
/// <param name="graphicsApi">The graphics API, valid during this call only.</param>
public delegate void RenderingNotifier(RenderingState state, GraphicsApi graphicsApi);

/// <summary>
/// Chooses the backend and renderer that Slint uses, such as one that renders with OpenGL for a <see cref="RenderingNotifier"/>.
/// Call <see cref="Select"/> before any other use of Slint, on the thread that then uses it.
/// </summary>
/// <example>
/// <code>
/// new BackendSelector().RequireOpenGLES().Select();
/// using var app = AppWindow.Create();
/// </code>
/// </example>
public sealed class BackendSelector
{
    // The `api` argument of slint_dotnet_backend_select in api/dotnet/native/rendering.rs.
    private int _api;
    private int _major = -1;
    private int _minor = -1;
    private string _backend = "";
    private string _renderer = "";

    /// <summary>Requires a renderer that uses OpenGL, of at least the version <paramref name="major"/>.<paramref name="minor"/> if given.</summary>
    public BackendSelector RequireOpenGL(int major = -1, int minor = 0) => Require(1, major, minor);

    /// <summary>Requires a renderer that uses OpenGL ES, of at least the version <paramref name="major"/>.<paramref name="minor"/> if given.</summary>
    public BackendSelector RequireOpenGLES(int major = -1, int minor = 0) => Require(2, major, minor);

    /// <summary>Requires a renderer that uses Metal.</summary>
    public BackendSelector RequireMetal() => Require(3, -1, -1);

    /// <summary>Requires a renderer that uses Vulkan.</summary>
    public BackendSelector RequireVulkan() => Require(4, -1, -1);

    /// <summary>Requires a renderer that uses Direct3D.</summary>
    public BackendSelector RequireD3D() => Require(5, -1, -1);

    /// <summary>Requires the backend <paramref name="name"/>, such as <c>winit</c> or <c>linuxkms</c>.</summary>
    public BackendSelector BackendName(string name)
    {
        ArgumentNullException.ThrowIfNull(name);
        _backend = name;
        return this;
    }

    /// <summary>Requires the renderer <paramref name="name"/>, such as <c>femtovg</c> or <c>skia</c>.</summary>
    public BackendSelector RendererName(string name)
    {
        ArgumentNullException.ThrowIfNull(name);
        _renderer = name;
        return this;
    }

    /// <summary>Creates the backend and renderer that meet the requirements.</summary>
    /// <exception cref="SlintException">
    /// No backend meets them, or Slint was used before; Slint then uses its default backend.
    /// </exception>
    public void Select()
    {
        using var backend = new PinnedUtf8(_backend);
        using var renderer = new PinnedUtf8(_renderer);
        NativeError.Check(NativeMethods.slint_dotnet_backend_select(_api, _major, _minor, backend.Str, renderer.Str));
    }

    private BackendSelector Require(int api, int major, int minor)
    {
        _api = api;
        _major = major;
        _minor = major < 0 ? -1 : minor;
        return this;
    }
}
