// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
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
    /// <summary><c>brush</c> or <c>color</c>, as <see cref="Color"/> for a solid color.</summary>
    Brush = 6,
    /// <summary><c>image</c>, as <see cref="OpaqueValue"/>.</summary>
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
            case OpaqueValue o:
                return NativeError.CheckValue(NativeMethods.slint_dotnet_value_clone(o.Handle.DangerousGetHandle()));
            case INativeModel model:
                return model.ToNativeValue();
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

    private static nint StructToNative(IEnumerable<KeyValuePair<string, object?>> fields)
    {
        var result = NativeError.CheckValue(NativeMethods.slint_dotnet_value_new_struct());
        try
        {
            foreach (var (name, fieldValue) in fields)
            {
                var field = ToNative(fieldValue);
                try
                {
                    using var fieldName = new PinnedUtf8(name);
                    NativeError.Check(NativeMethods.slint_dotnet_value_struct_set_field(result, fieldName.Str, field));
                }
                finally
                {
                    NativeMethods.slint_dotnet_value_free(field);
                }
            }
            return result;
        }
        catch
        {
            NativeMethods.slint_dotnet_value_free(result);
            throw;
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
                    return NativeMethods.slint_dotnet_value_to_color(value, &argb) ? Color.FromArgb(argb) : Opaque(value);
                }
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
        if (value is T t)
        {
            return t;
        }
        var target = Nullable.GetUnderlyingType(typeof(T)) ?? typeof(T);
        if (value is double number && target.IsPrimitive)
        {
            return (T)Convert.ChangeType(number, target, System.Globalization.CultureInfo.InvariantCulture);
        }
        if (value is null && default(T) is null)
        {
            return default!;
        }
        throw new InvalidCastException($"Can't convert {value?.GetType().Name ?? "null"} to {typeof(T).Name}");
    }

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
