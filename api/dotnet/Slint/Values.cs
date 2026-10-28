// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Collections.Concurrent;
using System.Runtime.CompilerServices;
using System.Runtime.InteropServices;
using System.Text;
using Slint.Interop;

namespace Slint;

/// <summary>The type of a property, as reported by <see cref="ComponentDefinition.Properties"/>.</summary>
public enum ValueType
{
    /// <summary>A type that the .NET API doesn't expose yet.</summary>
    Other = -1,
    /// <summary>No value.</summary>
    Void = 0,
    /// <summary><c>int</c>, <c>float</c>, and unit types such as <c>length</c>, as <see cref="double"/>.</summary>
    Number = 1,
    /// <summary><c>string</c>, as <see cref="string"/>.</summary>
    String = 2,
    /// <summary><c>bool</c>, as <see cref="bool"/>.</summary>
    Bool = 3,
    /// <summary>An array or model, as <see cref="Model{T}"/>.</summary>
    Model = 4,
    /// <summary>A struct, as <see cref="Slint.Struct"/>.</summary>
    Struct = 5,
    /// <summary><c>brush</c> or <c>color</c>, as <see cref="Color"/> for a solid color and <see cref="Slint.Brush"/> for a gradient.</summary>
    Brush = 6,
    /// <summary><c>image</c>, as <see cref="Slint.Image"/>.</summary>
    Image = 7,
}

/// <summary>An RGBA color with 8 bits per channel.</summary>
public readonly record struct Color(byte Red, byte Green, byte Blue, byte Alpha = 255)
{
    /// <summary>Creates a color from <c>0xAARRGGBB</c>.</summary>
    public static Color FromArgb(uint argb) =>
        new((byte)(argb >> 16), (byte)(argb >> 8), (byte)argb, (byte)(argb >> 24));

    /// <summary>Returns the color as <c>0xAARRGGBB</c>.</summary>
    public uint ToArgb() => (uint)Alpha << 24 | (uint)Red << 16 | (uint)Green << 8 | Blue;

    /// <summary>Returns a brighter color; a <paramref name="factor"/> of 0.5 is 50% brighter.</summary>
    public Color Brighter(float factor) => FromArgb(NativeMethods.slint_dotnet_color_brighter(ToArgb(), factor));

    /// <summary>Returns a darker color; a <paramref name="factor"/> of 0.5 is 50% darker.</summary>
    public Color Darker(float factor) => FromArgb(NativeMethods.slint_dotnet_color_darker(ToArgb(), factor));

    /// <summary>Returns a more transparent color; a <paramref name="factor"/> of 0.5 halves the opacity.</summary>
    public Color Transparentize(float factor) =>
        FromArgb(NativeMethods.slint_dotnet_color_transparentize(ToArgb(), factor));

    /// <summary>Mixes with <paramref name="other"/>; a <paramref name="factor"/> of 1 is this color, 0 the other.</summary>
    public Color Mix(Color other, float factor) =>
        FromArgb(NativeMethods.slint_dotnet_color_mix(ToArgb(), other.ToArgb(), factor));

    /// <summary>Returns the color with an opacity of <paramref name="alpha"/>, from 0 to 1.</summary>
    public Color WithAlpha(float alpha) => FromArgb(NativeMethods.slint_dotnet_color_with_alpha(ToArgb(), alpha));
}

/// <summary>
/// A <c>brush</c>: a solid color or a gradient.
/// Slint gives a solid brush as a <see cref="Color"/>, and a gradient as a <see cref="Brush"/>.
/// </summary>
public sealed class Brush : IEquatable<Brush>, IDisposable
{
    /// <summary>Creates a solid brush.</summary>
    public Brush(Color color)
        : this(new ValueHandle(NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_color(color.ToArgb()))))
    {
    }

    internal Brush(ValueHandle handle)
    {
        Handle = handle;
    }

    internal ValueHandle Handle { get; }

    /// <summary>Converts a color to a solid brush.</summary>
    public static implicit operator Brush(Color color) => new(color);

    /// <summary>The color of a solid brush, or the first color of a gradient.</summary>
    public Color Color => Color.FromArgb(Checked(NativeMethods.slint_dotnet_brush_color(Handle)));

    /// <summary>Whether the brush paints nothing.</summary>
    public bool IsTransparent => Checked(NativeMethods.slint_dotnet_brush_is_transparent(Handle));

    /// <summary>Whether the brush covers what's below it completely.</summary>
    public bool IsOpaque => Checked(NativeMethods.slint_dotnet_brush_is_opaque(Handle));

    /// <summary>Returns a brighter brush, see <see cref="Color.Brighter"/>.</summary>
    public Brush Brighter(float factor) => Derive(0, factor);

    /// <summary>Returns a darker brush, see <see cref="Color.Darker"/>.</summary>
    public Brush Darker(float factor) => Derive(1, factor);

    /// <summary>Returns a more transparent brush, see <see cref="Color.Transparentize"/>.</summary>
    public Brush Transparentize(float factor) => Derive(2, factor);

    /// <summary>Returns the brush with an opacity of <paramref name="alpha"/>, see <see cref="Color.WithAlpha"/>.</summary>
    public Brush WithAlpha(float alpha) => Derive(3, alpha);

    /// <inheritdoc/>
    public bool Equals(Brush? other) =>
        other is not null && Checked(NativeMethods.slint_dotnet_value_equals(Handle, other.Handle));

    /// <inheritdoc/>
    public override bool Equals(object? obj) => obj is Brush other && Equals(other);

    /// <inheritdoc/>
    public override int GetHashCode() => Color.GetHashCode();

    /// <summary>Releases the native brush.</summary>
    public void Dispose() => Handle.Dispose();

    private Brush Derive(int operation, float factor) =>
        new(new ValueHandle(NativeError.CheckValue(NativeMethods.slint_dotnet_brush_derive(Handle, operation, factor))));

    private static T Checked<T>(T value)
    {
        if (NativeError.TakeMessage() is { } message)
        {
            throw new SlintException(message);
        }
        return value;
    }
}

/// <summary>A value of a Slint enum, such as <c>TextHorizontalAlignment.center</c>.</summary>
public readonly record struct EnumValue(string EnumName, string Value);

/// <summary>A Slint struct; field names are as declared in the <c>.slint</c> file.</summary>
public sealed class Struct : Dictionary<string, object?>
{
    /// <summary>Creates an empty struct.</summary>
    public Struct()
        : base(StringComparer.Ordinal)
    {
    }
}

/// <summary>An image, such as one loaded from a file with <see cref="Load"/>.</summary>
public sealed unsafe class Image : IDisposable
{
    /// <summary>Creates an empty image.</summary>
    public Image()
        : this(new ValueHandle(NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_image())))
    {
    }

    internal Image(ValueHandle handle)
    {
        Handle = handle;
    }

    internal ValueHandle Handle { get; }

    /// <summary>Loads the image file at <paramref name="path"/>.</summary>
    public static Image Load(string path)
    {
        using var pathStr = new PinnedUtf8(System.IO.Path.GetFullPath(path));
        return new Image(new ValueHandle(NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_image_from_path(pathStr.Str))));
    }

    /// <summary>Decodes an image file in memory, such as a PNG, JPEG, or SVG.</summary>
    /// <param name="data">The content of the file.</param>
    /// <param name="format">The lowercase file extension, such as <c>"png"</c>, or <c>null</c> to guess it from the data.</param>
    /// <exception cref="SlintException">The data isn't an image in a supported format.</exception>
    public static Image LoadFromData(ReadOnlySpan<byte> data, string? format = null)
    {
        using var formatStr = new PinnedUtf8(format ?? "");
        fixed (byte* ptr = data)
        {
            return new Image(new ValueHandle(NativeError.CheckValue(
                NativeMethods.slint_dotnet_value_new_image_from_data(ptr, (nuint)data.Length, formatStr.Str))));
        }
    }

    /// <summary>Creates an image from pixels of 4 bytes each, red, green, blue, and alpha, row by row.</summary>
    /// <exception cref="SlintException"><paramref name="pixels"/> doesn't have <c>width * height * 4</c> bytes.</exception>
    public static Image FromRgba8(int width, int height, ReadOnlySpan<byte> pixels) => FromPixels(width, height, pixels, alpha: true);

    /// <summary>Creates an opaque image from pixels of 3 bytes each, red, green, and blue, row by row.</summary>
    /// <exception cref="SlintException"><paramref name="pixels"/> doesn't have <c>width * height * 3</c> bytes.</exception>
    public static Image FromRgb8(int width, int height, ReadOnlySpan<byte> pixels) => FromPixels(width, height, pixels, alpha: false);

    private static Image FromPixels(int width, int height, ReadOnlySpan<byte> pixels, bool alpha)
    {
        ArgumentOutOfRangeException.ThrowIfNegative(width);
        ArgumentOutOfRangeException.ThrowIfNegative(height);
        fixed (byte* ptr = pixels)
        {
            return new Image(new ValueHandle(NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_image_from_pixels(
                ptr, (nuint)pixels.Length, (uint)width, (uint)height, alpha))));
        }
    }

    /// <summary>Returns the pixels, 4 bytes each, red, green, blue, and alpha, row by row.</summary>
    /// <exception cref="SlintException">The image has no pixels in memory, such as an SVG that hasn't been rendered.</exception>
    public byte[] ToRgba8()
    {
        var pixels = new byte[Width * Height * 4];
        fixed (byte* ptr = pixels)
        {
            NativeError.Check(NativeMethods.slint_dotnet_value_image_to_rgba8(Handle, ptr, (nuint)pixels.Length));
        }
        return pixels;
    }

    /// <summary>The file the image was loaded from, or <c>null</c>.</summary>
    public string? Path => NativeMethods.slint_dotnet_value_image_path(Handle).Take();

    /// <summary>The width in pixels.</summary>
    public int Width => (int)Size().Width;

    /// <summary>The height in pixels.</summary>
    public int Height => (int)Size().Height;

    private (uint Width, uint Height) Size()
    {
        uint width, height;
        NativeMethods.slint_dotnet_value_image_size(Handle, &width, &height);
        return (width, height);
    }

    /// <summary>Releases the native image.</summary>
    public void Dispose() => Handle.Dispose();
}

/// <summary>
/// A Slint value with no .NET equivalent yet, such as an image or a gradient.
/// Pass it back to Slint unchanged.
/// </summary>
public sealed class OpaqueValue : IDisposable
{
    internal OpaqueValue(ValueHandle handle)
    {
        Handle = handle;
    }

    internal ValueHandle Handle { get; }

    /// <summary>Releases the native value.</summary>
    public void Dispose() => Handle.Dispose();
}

/// <summary>Converts between .NET objects and native <c>Value</c>s.</summary>
internal static unsafe class ValueConverter
{
    private const int KindEnum = 8;
    private const int KindKeys = 9;
    private const int KindStyledText = 10;
    private const int KindDataTransfer = 11;

    /// <summary>Returns a new native value; the caller frees it.</summary>
    public static nint ToNative(object? value)
    {
        switch (value)
        {
            case null:
                return NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_void());
            case bool b:
                return NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_bool(b));
            case string s:
                {
                    using var str = new PinnedUtf8(s);
                    return NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_string(str.Str));
                }
            case Color c:
                return NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_color(c.ToArgb()));
            case EnumValue e:
                {
                    using var enumName = new PinnedUtf8(e.EnumName);
                    using var enumValue = new PinnedUtf8(e.Value);
                    return NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_enum(enumName.Str, enumValue.Str));
                }
            case TimeSpan duration:
                return NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_number(duration.TotalMilliseconds));
            case NativeValue native:
                return NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(native.Handle.DangerousGetHandle()));
            case Brush brush:
                return NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(brush.Handle.DangerousGetHandle()));
            case LogicalPosition position:
                return StructToNative([new("x", position.X), new("y", position.Y)]);
            case LogicalSize size:
                return StructToNative([new("width", size.Width), new("height", size.Height)]);
            case Image image:
                return NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(image.Handle.DangerousGetHandle()));
            case OpaqueValue o:
                return NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(o.Handle.DangerousGetHandle()));
            case INativeModel model:
                return model.ToNativeValue();
            case not System.Collections.IEnumerable when Generated.Conversion.TryToSlint(value, out var converted):
                return ToNative(converted);
            case IEnumerable<KeyValuePair<string, object?>> fields:
                return StructToNative(fields);
            case System.Collections.IEnumerable rows:
                return ModelToNative(rows);
            case sbyte or byte or short or ushort or int or uint or long or ulong or float or double or decimal:
                return NativeError.CheckValue(
                    NativeMethods.slint_dotnet_value_new_number(Convert.ToDouble(value, System.Globalization.CultureInfo.InvariantCulture)));
            default:
                throw new ArgumentException($"Slint has no value type for {value.GetType()}", nameof(value));
        }
    }

    private const int MaxCachedFieldNames = 4096;

    /// <summary>UTF-8 field names on the pinned object heap, so that a struct passes them without copying.</summary>
    private static readonly ConcurrentDictionary<string, byte[]> s_fieldNames = new();

    private static nint StructToNative(IEnumerable<KeyValuePair<string, object?>> fields)
    {
        if (!fields.TryGetNonEnumeratedCount(out var count))
        {
            var copy = fields.ToList();
            fields = copy;
            count = copy.Count;
        }
        List<byte[]>? uncachedNames = null;
        Span<Utf8Str> names = count <= 16 ? stackalloc Utf8Str[count] : new Utf8Str[count];
        Span<nint> values = count <= 16 ? stackalloc nint[count] : new nint[count];
        Span<double> numbers = count <= 16 ? stackalloc double[count] : new double[count];
        var filled = 0;
        try
        {
            foreach (var (name, value) in fields)
            {
                if (filled == count)
                {
                    throw new InvalidOperationException("The struct's fields changed while converting it");
                }
                if (!TryGetCachedFieldName(name, out var bytes))
                {
                    (uncachedNames ??= []).Add(bytes);
                }
                names[filled] = new Utf8Str((byte*)Unsafe.AsPointer(ref MemoryMarshal.GetArrayDataReference(bytes)), (nuint)bytes.Length);
                if (TryGetNumber(value, out var number))
                {
                    values[filled] = 0;
                    numbers[filled] = number;
                }
                else
                {
                    values[filled] = ToNative(value);
                    numbers[filled] = 0;
                }
                filled++;
            }
        }
        catch
        {
            foreach (var value in values[..filled])
            {
                if (value != 0)
                {
                    NativeMethods.slint_dotnet_value_free(value);
                }
            }
            throw;
        }
        fixed (Utf8Str* namesPtr = names)
        fixed (nint* valuesPtr = values)
        fixed (double* numbersPtr = numbers)
        {
            // The struct takes ownership of the values.
            var result = NativeMethods.slint_dotnet_value_new_struct(namesPtr, valuesPtr, numbersPtr, (nuint)filled);
            GC.KeepAlive(uncachedNames);
            return NativeError.CheckValue(result);
        }
    }

    /// <summary>Returns the pinned UTF-8 name, and whether the cache keeps it alive.</summary>
    private static bool TryGetCachedFieldName(string name, out byte[] bytes)
    {
        if (s_fieldNames.TryGetValue(name, out bytes!))
        {
            return true;
        }
        bytes = GC.AllocateUninitializedArray<byte>(Encoding.UTF8.GetByteCount(name), pinned: true);
        Encoding.UTF8.GetBytes(name, bytes);
        // Structs built from data, such as with a key per user, don't fill the cache.
        if (s_fieldNames.Count >= MaxCachedFieldNames)
        {
            return false;
        }
        bytes = s_fieldNames.GetOrAdd(name, bytes);
        return true;
    }

    /// <summary>The numbers that <see cref="ToNative"/> converts to a number, which a struct takes without a value.</summary>
    private static bool TryGetNumber(object? value, out double number)
    {
        switch (value)
        {
            case float f:
                number = f;
                return true;
            case double d:
                number = d;
                return true;
            case int i:
                number = i;
                return true;
            case TimeSpan duration:
                number = duration.TotalMilliseconds;
                return true;
            case sbyte or byte or short or ushort or uint or long or ulong or decimal:
                number = Convert.ToDouble(value, System.Globalization.CultureInfo.InvariantCulture);
                return true;
            default:
                number = 0;
                return false;
        }
    }

    private static nint ModelToNative(System.Collections.IEnumerable rows)
    {
        var values = new List<nint>();
        try
        {
            foreach (var row in rows)
            {
                values.Add(ToNative(row));
            }
        }
        catch
        {
            values.ForEach(NativeMethods.slint_dotnet_value_free);
            throw;
        }
        var array = values.ToArray();
        fixed (nint* ptr = array)
        {
            // The model takes ownership of the rows.
            return NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_model(ptr, (nuint)array.Length));
        }
    }

    /// <summary>Converts a native value without taking ownership of it.</summary>
    public static object? FromNative(nint value)
    {
        switch (NativeMethods.slint_dotnet_value_kind(value))
        {
            case (int)ValueType.Void:
                return null;
            case (int)ValueType.Number:
                {
                    double n;
                    NativeMethods.slint_dotnet_value_to_number(value, &n);
                    return n;
                }
            case (int)ValueType.String:
                return NativeMethods.slint_dotnet_value_to_string(value).Take();
            case (int)ValueType.Bool:
                {
                    bool b;
                    NativeMethods.slint_dotnet_value_to_bool(value, &b);
                    return b;
                }
            case (int)ValueType.Model:
                var managed = NativeMethods.slint_dotnet_value_managed_model(value);
                return managed != 0
                    ? GCHandle.FromIntPtr(managed).Target
                    : new NativeModel(new ValueHandle(NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(value))));
            case (int)ValueType.Struct:
                return StructFromNative(value);
            case (int)ValueType.Brush:
                {
                    uint argb;
                    // Typed as object, or the conditional would convert the color to a Brush.
                    return NativeMethods.slint_dotnet_value_to_color(value, &argb)
                    ? (object)Color.FromArgb(argb)
                    : new Brush(new ValueHandle(NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(value))));
                }
            case (int)ValueType.Image:
                return new Image(new ValueHandle(NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(value))));
            case KindKeys:
                return new Keys(Clone(value));
            case KindStyledText:
                return new StyledText(Clone(value));
            case KindDataTransfer:
                return new DataTransfer(Clone(value));
            case KindEnum:
                return new EnumValue(
                    NativeMethods.slint_dotnet_value_enum_name(value).Take()!,
                    NativeMethods.slint_dotnet_value_enum_value(value).Take()!);
            default:
                return Opaque(value);
        }
    }

    /// <summary>Converts a native value and frees it.</summary>
    public static object? TakeFromNative(nint value)
    {
        try
        {
            return FromNative(value);
        }
        finally
        {
            NativeMethods.slint_dotnet_value_free(value);
        }
    }

    private static ValueHandle Clone(nint value) =>
        new(NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(value)));

    private static OpaqueValue Opaque(nint value) =>
        new(new ValueHandle(NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(value))));

    private static Struct StructFromNative(nint value)
    {
        var result = new Struct();
        foreach (var name in NameList.TakeNames(NativeMethods.slint_dotnet_value_struct_field_names(value)) ?? [])
        {
            using var fieldName = new PinnedUtf8(name);
            var field = NativeMethods.slint_dotnet_value_struct_get_field(value, fieldName.Str);
            result[name] = field == 0 ? null : TakeFromNative(field);
        }
        return result;
    }

    /// <summary>Converts the result of <see cref="FromNative"/> to <typeparamref name="T"/>, such as a number to <see cref="int"/>.</summary>
    public static T ConvertTo<T>(object? value)
    {
        if (Generated.Conversion.From<T>.Convert is { } convert)
        {
            return convert(value);
        }
        if (value is T t)
        {
            return t;
        }
        if (typeof(T) == typeof(TimeSpan) && value is double milliseconds)
        {
            return (T)(object)TimeSpan.FromMilliseconds(milliseconds);
        }
        if (typeof(T) == typeof(Brush) && value is Color color)
        {
            return (T)(object)new Brush(color);
        }
        if (typeof(T) == typeof(LogicalPosition) && value is Struct position)
        {
            return (T)(object)new LogicalPosition(Field(position, "x"), Field(position, "y"));
        }
        if (typeof(T) == typeof(LogicalSize) && value is Struct size)
        {
            return (T)(object)new LogicalSize(Field(size, "width"), Field(size, "height"));
        }
        var target = Nullable.GetUnderlyingType(typeof(T)) ?? typeof(T);
        if (value is double number && target.IsPrimitive)
        {
            // Saturating and truncating, like Slint's own conversions to integers.
            object converted = target == typeof(int) ? int.CreateSaturating(number)
                : target == typeof(long) ? long.CreateSaturating(number)
                : target == typeof(uint) ? uint.CreateSaturating(number)
                : target == typeof(ulong) ? ulong.CreateSaturating(number)
                : target == typeof(short) ? short.CreateSaturating(number)
                : target == typeof(ushort) ? ushort.CreateSaturating(number)
                : target == typeof(byte) ? byte.CreateSaturating(number)
                : target == typeof(sbyte) ? sbyte.CreateSaturating(number)
                : Convert.ChangeType(number, target, System.Globalization.CultureInfo.InvariantCulture);
            return (T)converted;
        }
        if (value is null && default(T) is null)
        {
            return default!;
        }
        throw new InvalidCastException($"Can't convert {value?.GetType().Name ?? "null"} to {typeof(T).Name}");
    }

    private static float Field(Struct value, string name) =>
        value.TryGetValue(name, out var field) && field is double number ? (float)number : 0;

    /// <summary>Converts <paramref name="args"/> to native values for the duration of <paramref name="body"/>.</summary>
    public static T WithNativeArgs<T>(object?[] args, Func<nint[], T> body)
    {
        var native = new nint[args.Length];
        try
        {
            for (var i = 0; i < args.Length; i++)
            {
                native[i] = ToNative(args[i]);
            }
            return body(native);
        }
        finally
        {
            foreach (var v in native)
            {
                if (v != 0)
                {
                    NativeMethods.slint_dotnet_value_free(v);
                }
            }
        }
    }
}
