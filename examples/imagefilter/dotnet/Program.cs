// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using ImageFilter;
using Slint;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        using var original = Image.Load(Path.Combine(AppContext.BaseDirectory, "cat.jpg"));
        window.OriginalImage = original;
        var pixels = new Pixels(original.Width, original.Height, original.ToRgba8());

        (string Name, Func<Pixels, Pixels> Apply)[] filters =
        [
            ("Blur", image => image.Blur(sigma: 4)),
            ("Brighten", image => image.MapColors(value => value + 30)),
            ("Darken", image => image.MapColors(value => value - 30)),
            ("Increase Contrast", image => image.Contrast(30)),
            ("Decrease Contrast", image => image.Contrast(-30)),
            ("Invert", image => image.MapColors(value => 255 - value)),
        ];
        window.Filters = new ListModel<string>(filters.Select(filter => filter.Name));
        window.OnFilterImage(index => filters[index].Apply(pixels).ToImage());

        window.Run();
    }
}

/// <summary>An image as RGBA bytes, row by row, with filters that leave the alpha channel as it is.</summary>
internal sealed record Pixels(int Width, int Height, byte[] Rgba)
{
    public Image ToImage() => Image.FromRgba8(Width, Height, Rgba);

    /// <summary>Applies <paramref name="map"/> to the red, green, and blue of each pixel, clamped to 0–255.</summary>
    public Pixels MapColors(Func<int, int> map)
    {
        var result = (byte[])Rgba.Clone();
        for (var i = 0; i < result.Length; i++)
        {
            if (i % 4 != 3)
            {
                result[i] = (byte)Math.Clamp(map(result[i]), 0, 255);
            }
        }
        return this with { Rgba = result };
    }

    /// <summary>Scales the color values around the middle, like <c>contrast</c> of the Rust <c>image</c> crate.</summary>
    public Pixels Contrast(float contrast)
    {
        var factor = MathF.Pow((100 + contrast) / 100, 2);
        return MapColors(value => (int)MathF.Round(((value / 255f - 0.5f) * factor + 0.5f) * 255));
    }

    /// <summary>A Gaussian blur, in a horizontal and a vertical pass.</summary>
    public Pixels Blur(float sigma)
    {
        var radius = (int)Math.Ceiling(sigma * 3);
        var kernel = Enumerable.Range(-radius, 2 * radius + 1).Select(x => MathF.Exp(-x * x / (2 * sigma * sigma))).ToArray();
        var sum = kernel.Sum();
        for (var i = 0; i < kernel.Length; i++)
        {
            kernel[i] /= sum;
        }
        return Convolve(kernel, horizontal: true).Convolve(kernel, horizontal: false);
    }

    private Pixels Convolve(float[] kernel, bool horizontal)
    {
        var radius = kernel.Length / 2;
        var result = (byte[])Rgba.Clone();
        for (var y = 0; y < Height; y++)
        {
            for (var x = 0; x < Width; x++)
            {
                for (var channel = 0; channel < 3; channel++)
                {
                    var value = 0f;
                    for (var k = -radius; k <= radius; k++)
                    {
                        // Clamping at the border repeats the edge pixels.
                        var sx = horizontal ? Math.Clamp(x + k, 0, Width - 1) : x;
                        var sy = horizontal ? y : Math.Clamp(y + k, 0, Height - 1);
                        value += kernel[k + radius] * Rgba[(sy * Width + sx) * 4 + channel];
                    }
                    result[(y * Width + x) * 4 + channel] = (byte)Math.Clamp(MathF.Round(value), 0, 255);
                }
            }
        }
        return this with { Rgba = result };
    }
}
