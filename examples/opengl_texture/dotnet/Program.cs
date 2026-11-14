// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Diagnostics;
using OpenGLTexture;
using Slint;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        try
        {
            new BackendSelector().RequireOpenGL().Select();
        }
        catch (SlintException e)
        {
            Console.Error.WriteLine($"Rendering with OpenGL isn't available, so the window shows no texture: {e.Message}");
        }

        using var app = App.Create();
        DemoRenderer? renderer = null;
        var weakApp = new WeakReference<App>(app);
        try
        {
            app.Window.SetRenderingNotifier((state, api) =>
            {
                switch (state)
                {
                    case RenderingState.RenderingSetup when api.Kind == GraphicsApiKind.NativeOpenGL:
                        renderer = new DemoRenderer(new GL(api));
                        break;
                    case RenderingState.BeforeRendering when renderer != null && weakApp.TryGetTarget(out var target):
                        target.Texture = renderer.Render(
                            target.SelectedRed,
                            target.SelectedGreen,
                            target.SelectedBlue,
                            target.RequestedTextureWidth,
                            target.RequestedTextureHeight);
                        // The cube keeps turning, so render the next frame too.
                        target.Window.RequestRedraw();
                        break;
                    case RenderingState.RenderingTeardown:
                        renderer?.Dispose();
                        renderer = null;
                        break;
                }
            });
        }
        catch (NotSupportedException e)
        {
            Console.Error.WriteLine(e.Message);
        }
        app.Run();
    }
}

/// <summary>A texture and the framebuffer that renders into it.</summary>
internal sealed class DemoTexture : IDisposable
{
    private readonly GL _gl;
    private readonly uint _framebuffer;

    public DemoTexture(GL gl, int width, int height)
    {
        _gl = gl;
        Width = width;
        Height = height;
        Texture = gl.GenTexture();
        var oldTexture = (uint)gl.GetInteger(GL.TextureBinding2D);
        gl.BindTexture(GL.Texture2D, Texture);
        gl.TexParameter(GL.Texture2D, GL.TextureMinFilter, GL.Linear);
        gl.TexParameter(GL.Texture2D, GL.TextureMagFilter, GL.Linear);
        gl.TexParameter(GL.Texture2D, GL.TextureWrapS, GL.ClampToEdge);
        gl.TexParameter(GL.Texture2D, GL.TextureWrapT, GL.ClampToEdge);
        gl.TexImage2D(GL.Texture2D, width, height);

        _framebuffer = gl.GenFramebuffer();
        var oldFramebuffer = (uint)gl.GetInteger(GL.FramebufferBinding);
        gl.BindFramebuffer(GL.Framebuffer, _framebuffer);
        gl.FramebufferTexture2D(GL.Framebuffer, GL.ColorAttachment0, GL.Texture2D, Texture);
        Debug.Assert(gl.CheckFramebufferStatus(GL.Framebuffer) == GL.FramebufferComplete);
        gl.BindFramebuffer(GL.Framebuffer, oldFramebuffer);
        gl.BindTexture(GL.Texture2D, oldTexture);
    }

    public uint Texture { get; }

    public int Width { get; }

    public int Height { get; }

    /// <summary>Runs <paramref name="draw"/> with the texture as the framebuffer to draw into.</summary>
    public void DrawInto(Action draw)
    {
        var oldFramebuffer = (uint)_gl.GetInteger(GL.FramebufferBinding);
        _gl.BindFramebuffer(GL.Framebuffer, _framebuffer);
        draw();
        _gl.BindFramebuffer(GL.Framebuffer, oldFramebuffer);
    }

    public void Dispose()
    {
        _gl.DeleteFramebuffer(_framebuffer);
        _gl.DeleteTexture(Texture);
    }
}

/// <summary>Renders a turning, lit cube into a texture, and shows the previous frame's texture while it renders the next.</summary>
internal sealed class DemoRenderer : IDisposable
{
    private const string VertexShader = """
        in vec2 position;
        out vec2 frag_position;
        void main() {
            frag_position = position;
            gl_Position = vec4(position, 0.0, 1.0);
        }
        """;

    private const string FragmentShader = """
        in vec2 frag_position;
        out vec4 frag_color;
        uniform vec3 selected_light_color;
        uniform float iTime;

        float sdRoundBox(vec3 p, vec3 b, float r)
        {
            vec3 q = abs(p) - b;
            return length(max(q,0.0)) + min(max(q.x,max(q.y,q.z)),0.0) - r;
        }

        vec3 rotateY(vec3 r, float angle)
        {
            mat3 rotation_matrix = mat3(cos(angle), 0, sin(angle), 0, 1, 0, -sin(angle), 0, cos(angle));
            return rotation_matrix * r;
        }

        vec3 rotateZ(vec3 r, float angle) {
            mat3 rotation_matrix = mat3(cos(angle), -sin(angle), 0, sin(angle), cos(angle), 0, 0, 0, 1);
            return rotation_matrix * r;
        }

        float scene(vec3 r)
        {
            vec3 pos = rotateZ(rotateY(r + vec3(-1.0, -1.0, 4.0), iTime), iTime);
            vec3 cube = vec3(0.5, 0.5, 0.5);
            float edge = 0.1;
            return sdRoundBox(pos, cube, edge);
        }

        vec3 normal( in vec3 pos )
        {
            vec2 e = vec2(1.0,-1.0)*0.5773;
            const float eps = 0.0005;
            return normalize( e.xyy*scene( pos + e.xyy*eps ) +
                            e.yyx*scene( pos + e.yyx*eps ) +
                            e.yxy*scene( pos + e.yxy*eps ) +
                            e.xxx*scene( pos + e.xxx*eps ) );
        }

        #define ITERATIONS 90
        #define EPS 0.0001

        vec4 render(vec2 fragCoord, vec3 light_color)
        {
            vec4 color = vec4(0, 0, 0, 1);

            vec3 camera = vec3(1.0, 2.0, 1.0);
            vec3 p = vec3(fragCoord.x, fragCoord.y + 1.0, -1.0);
            vec3 dir = normalize(p - camera);

            for(int i=0; i < ITERATIONS; i++)
            {
                float dist = scene(p);
                if(dist < EPS) {
                    break;
                }
                p = p + dir * dist;
            }

            vec3 surf_normal = normal(p);

            vec3 light_position = vec3(2.0, 4.0, -0.5);
            float light = 7.0 + 2.0 * dot(surf_normal, light_position);
            light /= 0.2 * pow(length(light_position - p), 3.5);

            return vec4(light * light_color.x, light * light_color.y, light * light_color.z, 1.0) * 2.0;
        }

        void main() {
            vec2 r = vec2(0.5 * frag_position.x + 1.0, 0.5 - 0.5 * frag_position.y);
            frag_color = render(r, selected_light_color);
        }
        """;

    private readonly GL _gl;
    private readonly uint _program;
    private readonly uint _vbo;
    private readonly uint _vao;
    private readonly int _effectTimeLocation;
    private readonly int _selectedLightColorLocation;
    private readonly Stopwatch _stopwatch = Stopwatch.StartNew();
    private DemoTexture _displayedTexture;
    private DemoTexture _nextTexture;

    public DemoRenderer(GL gl)
    {
        _gl = gl;
        // OpenGL ES and desktop OpenGL take different GLSL versions of the same shaders.
        var header = gl.GetString(GL.Version).Contains("OpenGL ES", StringComparison.Ordinal)
            ? "#version 300 es\nprecision highp float;\n"
            : "#version 150\n";
        _program = gl.CreateProgram();
        var vertexShader = gl.AttachShader(_program, GL.VertexShader, header + VertexShader);
        var fragmentShader = gl.AttachShader(_program, GL.FragmentShader, header + FragmentShader);
        gl.LinkProgram(_program);
        gl.DetachAndDeleteShader(_program, vertexShader);
        gl.DetachAndDeleteShader(_program, fragmentShader);

        _effectTimeLocation = gl.GetUniformLocation(_program, "iTime");
        _selectedLightColorLocation = gl.GetUniformLocation(_program, "selected_light_color");
        var positionLocation = gl.GetAttribLocation(_program, "position");

        _vbo = gl.GenBuffer();
        gl.BindBuffer(GL.ArrayBuffer, _vbo);
        gl.BufferData(GL.ArrayBuffer, [-1f, 1f, -1f, -1f, 1f, 1f, 1f, -1f], GL.StaticDraw);
        _vao = gl.GenVertexArray();
        gl.BindVertexArray(_vao);
        gl.EnableVertexAttribArray(positionLocation);
        gl.VertexAttribPointer(positionLocation, 2, GL.Float, false, 8, 0);
        gl.BindBuffer(GL.ArrayBuffer, 0);
        gl.BindVertexArray(0);

        _displayedTexture = new DemoTexture(gl, 320, 200);
        _nextTexture = new DemoTexture(gl, 320, 200);
    }

    /// <summary>Renders the next frame, and returns it as an image that Slint shows until the next call.</summary>
    public Image Render(float red, float green, float blue, int width, int height)
    {
        // Slint renders with the same context, so restore what this changes.
        var oldBuffer = (uint)_gl.GetInteger(GL.ArrayBufferBinding);
        var oldVertexArray = (uint)_gl.GetInteger(GL.VertexArrayBinding);
        _gl.UseProgram(_program);
        _gl.BindBuffer(GL.ArrayBuffer, _vbo);
        _gl.BindVertexArray(_vao);

        if (_nextTexture.Width != width || _nextTexture.Height != height)
        {
            _nextTexture.Dispose();
            _nextTexture = new DemoTexture(_gl, width, height);
        }
        _nextTexture.DrawInto(() =>
        {
            var viewport = _gl.GetViewport();
            _gl.SetViewport(0, 0, _nextTexture.Width, _nextTexture.Height);
            _gl.Uniform1(_effectTimeLocation, (float)_stopwatch.Elapsed.TotalMilliseconds / 500);
            _gl.Uniform3(_selectedLightColorLocation, red, green, blue);
            _gl.DrawArrays(GL.TriangleStrip, 0, 4);
            _gl.SetViewport(viewport.X, viewport.Y, viewport.Width, viewport.Height);
        });

        _gl.BindBuffer(GL.ArrayBuffer, oldBuffer);
        _gl.BindVertexArray(oldVertexArray);
        _gl.UseProgram(0);

        var image = Image.FromBorrowedOpenGLTexture(_nextTexture.Texture, _nextTexture.Width, _nextTexture.Height);
        (_nextTexture, _displayedTexture) = (_displayedTexture, _nextTexture);
        return image;
    }

    public void Dispose()
    {
        _gl.DeleteProgram(_program);
        _gl.DeleteVertexArray(_vao);
        _gl.DeleteBuffer(_vbo);
        _displayedTexture.Dispose();
        _nextTexture.Dispose();
    }
}
