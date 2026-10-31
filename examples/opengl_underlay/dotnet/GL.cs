// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Runtime.InteropServices;
using System.Text;
using Slint;

namespace OpenGLUnderlay;

/// <summary>The OpenGL functions that the underlay draws with, loaded from the window's context.</summary>
internal sealed unsafe class GL
{
    public const uint ArrayBuffer = 0x8892;
    public const uint StaticDraw = 0x88E4;
    public const uint Float = 0x1406;
    public const uint TriangleStrip = 0x0005;
    public const uint VertexShader = 0x8B31;
    public const uint FragmentShader = 0x8B30;
    public const uint CompileStatus = 0x8B81;
    public const uint LinkStatus = 0x8B82;
    public const uint ArrayBufferBinding = 0x8894;
    public const uint VertexArrayBinding = 0x85B5;
    public const uint Version = 0x1F02;

    private readonly delegate* unmanaged<uint> _createProgram;
    private readonly delegate* unmanaged<uint, uint> _createShader;
    private readonly delegate* unmanaged<uint, int, byte**, int*, void> _shaderSource;
    private readonly delegate* unmanaged<uint, void> _compileShader;
    private readonly delegate* unmanaged<uint, uint, int*, void> _getShaderiv;
    private readonly delegate* unmanaged<uint, int, int*, byte*, void> _getShaderInfoLog;
    private readonly delegate* unmanaged<uint, uint, void> _attachShader;
    private readonly delegate* unmanaged<uint, uint, void> _detachShader;
    private readonly delegate* unmanaged<uint, void> _deleteShader;
    private readonly delegate* unmanaged<uint, void> _linkProgram;
    private readonly delegate* unmanaged<uint, uint, int*, void> _getProgramiv;
    private readonly delegate* unmanaged<uint, int, int*, byte*, void> _getProgramInfoLog;
    private readonly delegate* unmanaged<uint, void> _useProgram;
    private readonly delegate* unmanaged<uint, void> _deleteProgram;
    private readonly delegate* unmanaged<uint, byte*, int> _getUniformLocation;
    private readonly delegate* unmanaged<uint, byte*, int> _getAttribLocation;
    private readonly delegate* unmanaged<int, float, void> _uniform1f;
    private readonly delegate* unmanaged<int, uint*, void> _genBuffers;
    private readonly delegate* unmanaged<int, uint*, void> _deleteBuffers;
    private readonly delegate* unmanaged<uint, uint, void> _bindBuffer;
    private readonly delegate* unmanaged<uint, nint, void*, uint, void> _bufferData;
    private readonly delegate* unmanaged<int, uint*, void> _genVertexArrays;
    private readonly delegate* unmanaged<int, uint*, void> _deleteVertexArrays;
    private readonly delegate* unmanaged<uint, void> _bindVertexArray;
    private readonly delegate* unmanaged<uint, void> _enableVertexAttribArray;
    private readonly delegate* unmanaged<uint, int, uint, byte, int, nint, void> _vertexAttribPointer;
    private readonly delegate* unmanaged<uint, int, int, void> _drawArrays;
    private readonly delegate* unmanaged<uint, int*, void> _getIntegerv;
    private readonly delegate* unmanaged<uint, byte*> _getString;

    public GL(GraphicsApi api)
    {
        _createProgram = (delegate* unmanaged<uint>)Load(api, "glCreateProgram");
        _createShader = (delegate* unmanaged<uint, uint>)Load(api, "glCreateShader");
        _shaderSource = (delegate* unmanaged<uint, int, byte**, int*, void>)Load(api, "glShaderSource");
        _compileShader = (delegate* unmanaged<uint, void>)Load(api, "glCompileShader");
        _getShaderiv = (delegate* unmanaged<uint, uint, int*, void>)Load(api, "glGetShaderiv");
        _getShaderInfoLog = (delegate* unmanaged<uint, int, int*, byte*, void>)Load(api, "glGetShaderInfoLog");
        _attachShader = (delegate* unmanaged<uint, uint, void>)Load(api, "glAttachShader");
        _detachShader = (delegate* unmanaged<uint, uint, void>)Load(api, "glDetachShader");
        _deleteShader = (delegate* unmanaged<uint, void>)Load(api, "glDeleteShader");
        _linkProgram = (delegate* unmanaged<uint, void>)Load(api, "glLinkProgram");
        _getProgramiv = (delegate* unmanaged<uint, uint, int*, void>)Load(api, "glGetProgramiv");
        _getProgramInfoLog = (delegate* unmanaged<uint, int, int*, byte*, void>)Load(api, "glGetProgramInfoLog");
        _useProgram = (delegate* unmanaged<uint, void>)Load(api, "glUseProgram");
        _deleteProgram = (delegate* unmanaged<uint, void>)Load(api, "glDeleteProgram");
        _getUniformLocation = (delegate* unmanaged<uint, byte*, int>)Load(api, "glGetUniformLocation");
        _getAttribLocation = (delegate* unmanaged<uint, byte*, int>)Load(api, "glGetAttribLocation");
        _uniform1f = (delegate* unmanaged<int, float, void>)Load(api, "glUniform1f");
        _genBuffers = (delegate* unmanaged<int, uint*, void>)Load(api, "glGenBuffers");
        _deleteBuffers = (delegate* unmanaged<int, uint*, void>)Load(api, "glDeleteBuffers");
        _bindBuffer = (delegate* unmanaged<uint, uint, void>)Load(api, "glBindBuffer");
        _bufferData = (delegate* unmanaged<uint, nint, void*, uint, void>)Load(api, "glBufferData");
        _genVertexArrays = (delegate* unmanaged<int, uint*, void>)Load(api, "glGenVertexArrays");
        _deleteVertexArrays = (delegate* unmanaged<int, uint*, void>)Load(api, "glDeleteVertexArrays");
        _bindVertexArray = (delegate* unmanaged<uint, void>)Load(api, "glBindVertexArray");
        _enableVertexAttribArray = (delegate* unmanaged<uint, void>)Load(api, "glEnableVertexAttribArray");
        _vertexAttribPointer = (delegate* unmanaged<uint, int, uint, byte, int, nint, void>)Load(api, "glVertexAttribPointer");
        _drawArrays = (delegate* unmanaged<uint, int, int, void>)Load(api, "glDrawArrays");
        _getIntegerv = (delegate* unmanaged<uint, int*, void>)Load(api, "glGetIntegerv");
        _getString = (delegate* unmanaged<uint, byte*>)Load(api, "glGetString");
    }

    public string GetString(uint name) => Marshal.PtrToStringUTF8((nint)_getString(name)) ?? "";

    public int GetInteger(uint name)
    {
        int value;
        _getIntegerv(name, &value);
        return value;
    }

    public uint CreateProgram() => _createProgram();

    public void UseProgram(uint program) => _useProgram(program);

    public void DeleteProgram(uint program) => _deleteProgram(program);

    /// <summary>Compiles <paramref name="source"/> and attaches it to <paramref name="program"/>; throws with the compiler's log on errors.</summary>
    public uint AttachShader(uint program, uint type, string source)
    {
        var shader = _createShader(type);
        var bytes = Encoding.UTF8.GetBytes(source);
        fixed (byte* ptr = bytes)
        {
            var length = bytes.Length;
            var sources = ptr;
            _shaderSource(shader, 1, &sources, &length);
        }
        _compileShader(shader);
        int compiled;
        _getShaderiv(shader, CompileStatus, &compiled);
        if (compiled == 0)
        {
            var log = new byte[4096];
            int length;
            fixed (byte* ptr = log)
            {
                _getShaderInfoLog(shader, log.Length, &length, ptr);
            }
            _deleteShader(shader);
            throw new InvalidOperationException("Compiling a shader failed: " + Encoding.UTF8.GetString(log, 0, length));
        }
        _attachShader(program, shader);
        return shader;
    }

    public void LinkProgram(uint program)
    {
        _linkProgram(program);
        int linked;
        _getProgramiv(program, LinkStatus, &linked);
        if (linked == 0)
        {
            var log = new byte[4096];
            int length;
            fixed (byte* ptr = log)
            {
                _getProgramInfoLog(program, log.Length, &length, ptr);
            }
            throw new InvalidOperationException("Linking the shaders failed: " + Encoding.UTF8.GetString(log, 0, length));
        }
    }

    public void DetachAndDeleteShader(uint program, uint shader)
    {
        _detachShader(program, shader);
        _deleteShader(shader);
    }

    public int GetUniformLocation(uint program, string name)
    {
        fixed (byte* ptr = Encoding.UTF8.GetBytes(name + "\0"))
        {
            return _getUniformLocation(program, ptr);
        }
    }

    public uint GetAttribLocation(uint program, string name)
    {
        fixed (byte* ptr = Encoding.UTF8.GetBytes(name + "\0"))
        {
            return (uint)_getAttribLocation(program, ptr);
        }
    }

    public void Uniform1(int location, float value) => _uniform1f(location, value);

    public uint GenBuffer()
    {
        uint buffer;
        _genBuffers(1, &buffer);
        return buffer;
    }

    public void DeleteBuffer(uint buffer) => _deleteBuffers(1, &buffer);

    public void BindBuffer(uint target, uint buffer) => _bindBuffer(target, buffer);

    public void BufferData(uint target, ReadOnlySpan<float> data, uint usage)
    {
        fixed (float* ptr = data)
        {
            _bufferData(target, data.Length * sizeof(float), ptr, usage);
        }
    }

    public uint GenVertexArray()
    {
        uint array;
        _genVertexArrays(1, &array);
        return array;
    }

    public void DeleteVertexArray(uint array) => _deleteVertexArrays(1, &array);

    public void BindVertexArray(uint array) => _bindVertexArray(array);

    public void EnableVertexAttribArray(uint index) => _enableVertexAttribArray(index);

    public void VertexAttribPointer(uint index, int size, uint type, bool normalized, int stride, nint offset) =>
        _vertexAttribPointer(index, size, type, normalized ? (byte)1 : (byte)0, stride, offset);

    public void DrawArrays(uint mode, int first, int count) => _drawArrays(mode, first, count);

    private static nint Load(GraphicsApi api, string name)
    {
        var address = api.GetProcAddress(name);
        return address != 0 ? address : throw new InvalidOperationException($"OpenGL has no {name}");
    }
}
