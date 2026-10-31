// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Diagnostics;
using OpenGLUnderlay;
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
            Console.Error.WriteLine($"Rendering with OpenGL isn't available, so the window shows no underlay: {e.Message}");
        }

        using var app = App.Create();
        Underlay? underlay = null;
        var weakApp = new WeakReference<App>(app);
        try
        {
            app.Window.SetRenderingNotifier((state, api) =>
            {
                switch (state)
                {
                    case RenderingState.RenderingSetup when api.Kind == GraphicsApiKind.NativeOpenGL:
                        underlay = new Underlay(new GL(api));
                        break;
                    case RenderingState.BeforeRendering when underlay != null && weakApp.TryGetTarget(out var target):
                        underlay.Render(target.RotationEnabled);
                        // The effect is continuous, so render the next frame too.
                        target.Window.RequestRedraw();
                        break;
                    case RenderingState.RenderingTeardown:
                        underlay?.Dispose();
                        underlay = null;
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

/// <summary>Draws an animated pattern of diamonds behind the scene, with a shader.</summary>
internal sealed class Underlay : IDisposable
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
        uniform float effect_time;
        uniform float rotation_time;
        const vec3 COLOR_BG_DARK   = vec3(0.106, 0.106, 0.118);
        const vec3 COLOR_DIAMOND   = vec3(0.137, 0.149, 0.184);
        const vec3 COLOR_ACCENT    = vec3(0.12, 0.35, 0.75);
        mat2 rotate(float angle) {
            float s = sin(angle);
            float c = cos(angle);
            return mat2(c, -s, s, c);
        }
        void main() {
            vec2 p_coords = frag_position;
            float perspective_strength = 0.09;
            float divisor = 1.0 + (-p_coords.y + 1.0) * perspective_strength;
            p_coords /= divisor;
            p_coords.y *= (1.0 + perspective_strength * 1.5);
            const float MAX_ANGLE_DEGREES = 10.0;
            float max_angle_rad = radians(MAX_ANGLE_DEGREES);
            float oscillating_factor = sin(rotation_time / 1700.0);
            float angle = oscillating_factor * max_angle_rad;
            mat2 rotation_matrix = rotate(angle);
            vec2 uv = rotation_matrix * p_coords * 6.0;
            vec2 grid_id = floor(uv);
            vec2 grid_uv = fract(uv) - 0.5;
            float manhattan_dist = abs(grid_uv.x) + abs(grid_uv.y);
            float wave_time = effect_time / 300.0;
            float wave_offset = grid_id.x * 0.5 + grid_id.y * 0.15;
            float accent_alpha = 0.5 + 0.5 * sin(wave_time + wave_offset);
            accent_alpha = pow(accent_alpha, 2.0);
            float diamond_size = 0.5;
            float border_thickness = 0.03;
            float diamond_fill_mask = 1.0 - smoothstep(diamond_size, diamond_size, manhattan_dist);
            float border_glow_mask = smoothstep(diamond_size - border_thickness, diamond_size, manhattan_dist) -
                                     smoothstep(diamond_size, diamond_size + border_thickness, manhattan_dist);
            vec3 final_color = COLOR_BG_DARK;
            final_color = mix(final_color, COLOR_DIAMOND, diamond_fill_mask);
            final_color = mix(final_color, COLOR_ACCENT, border_glow_mask * accent_alpha);
            frag_color = vec4(final_color, 1.0);
        }
        """;

    private readonly GL _gl;
    private readonly uint _program;
    private readonly int _effectTimeLocation;
    private readonly int _rotationTimeLocation;
    private readonly uint _vbo;
    private readonly uint _vao;
    private readonly Stopwatch _stopwatch = Stopwatch.StartNew();
    private float _rotationTime;
    private bool _lastRotationEnabled = true;
    private float _rotationPauseOffset;

    public Underlay(GL gl)
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

        _effectTimeLocation = gl.GetUniformLocation(_program, "effect_time");
        _rotationTimeLocation = gl.GetUniformLocation(_program, "rotation_time");
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
    }

    public void Render(bool rotationEnabled)
    {
        // Slint renders with the same context, so restore what this changes.
        var oldBuffer = (uint)_gl.GetInteger(GL.ArrayBufferBinding);
        var oldVertexArray = (uint)_gl.GetInteger(GL.VertexArrayBinding);
        _gl.UseProgram(_program);
        _gl.BindBuffer(GL.ArrayBuffer, _vbo);
        _gl.BindVertexArray(_vao);

        var elapsed = (float)_stopwatch.Elapsed.TotalMilliseconds;
        _gl.Uniform1(_effectTimeLocation, elapsed);
        // Pausing the rotation keeps its angle, and resuming continues from there.
        if (rotationEnabled)
        {
            if (!_lastRotationEnabled)
            {
                _rotationPauseOffset = elapsed - _rotationTime;
            }
            _rotationTime = elapsed - _rotationPauseOffset;
        }
        _gl.Uniform1(_rotationTimeLocation, _rotationTime);
        _lastRotationEnabled = rotationEnabled;

        _gl.DrawArrays(GL.TriangleStrip, 0, 4);

        _gl.BindBuffer(GL.ArrayBuffer, oldBuffer);
        _gl.BindVertexArray(oldVertexArray);
        _gl.UseProgram(0);
    }

    public void Dispose()
    {
        _gl.DeleteProgram(_program);
        _gl.DeleteVertexArray(_vao);
        _gl.DeleteBuffer(_vbo);
    }
}
