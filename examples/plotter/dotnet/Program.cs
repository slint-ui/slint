// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Numerics;
using Plotter;
using Slint;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        window.OnRenderPlot((pitch, yaw, amplitude) => SurfacePlot.Render(640, 480, pitch, yaw, amplitude));
        window.Run();
    }
}

/// <summary>Draws the surface of a 2D Gaussian into an RGB image, the way the Rust version does with the plotters crate.</summary>
internal static class SurfacePlot
{
    private const int GridSteps = 30;

    public static Image Render(int width, int height, float pitch, float yaw, float amplitude)
    {
        var canvas = new Canvas(width, height);
        // The plot spans x and z from -3 to 3, and y from 0 to 6; the projection centers and rotates that box.
        var rotation = Matrix4x4.CreateRotationY(yaw) * Matrix4x4.CreateRotationX(pitch);
        var scale = 0.7f * Math.Min(width, height) / 6f;
        Vector3 Project(float x, float y, float z)
        {
            var p = Vector3.Transform(new Vector3(x, y - 3, z), rotation);
            return new Vector3(width / 2f + p.X * scale, height / 2f - p.Y * scale, p.Z);
        }

        // The edges of the box, as axes.
        var gray = (r: (byte)180, g: (byte)180, b: (byte)180);
        foreach (var (a, b) in BoxEdges())
        {
            canvas.Line(Project(a.X, a.Y, a.Z), Project(b.X, b.Y, b.Z), gray);
        }

        // The quads of the surface, from the farthest to the nearest.
        var points = new Vector3[GridSteps + 1, GridSteps + 1];
        var heights = new float[GridSteps + 1, GridSteps + 1];
        for (var i = 0; i <= GridSteps; i++)
        {
            for (var j = 0; j <= GridSteps; j++)
            {
                var x = -3 + 6f * i / GridSteps;
                var z = -3 + 6f * j / GridSteps;
                heights[i, j] = Pdf(x, z, amplitude);
                points[i, j] = Project(x, heights[i, j], z);
            }
        }
        var quads = new List<(float Depth, Vector3[] Corners, float Height)>();
        for (var i = 0; i < GridSteps; i++)
        {
            for (var j = 0; j < GridSteps; j++)
            {
                Vector3[] corners = [points[i, j], points[i + 1, j], points[i + 1, j + 1], points[i, j + 1]];
                var meanHeight = (heights[i, j] + heights[i + 1, j] + heights[i + 1, j + 1] + heights[i, j + 1]) / 4;
                quads.Add((corners.Average(c => c.Z), corners, meanHeight));
            }
        }
        foreach (var quad in quads.OrderBy(q => q.Depth))
        {
            var color = Hsl(240f / 360 - 240f / 360 * quad.Height / 5, 1, 0.7f);
            canvas.FillTriangle(quad.Corners[0], quad.Corners[1], quad.Corners[2], color);
            canvas.FillTriangle(quad.Corners[0], quad.Corners[2], quad.Corners[3], color);
        }
        return Image.FromRgb8(width, height, canvas.Rgb);
    }

    private static float Pdf(float x, float y, float amplitude)
    {
        const float Deviation = 0.1f;
        x /= 10;
        y /= 10;
        return amplitude * MathF.Exp(-x * x / 2 / Deviation / Deviation - y * y / 2 / Deviation / Deviation);
    }

    private static IEnumerable<(Vector3, Vector3)> BoxEdges()
    {
        Vector3[] corners = [.. from x in new[] { -3f, 3f } from y in new[] { 0f, 6f } from z in new[] { -3f, 3f } select new Vector3(x, y, z)];
        // Two corners make an edge when they differ in one coordinate.
        return from a in corners
               from b in corners
               where a.X <= b.X && a.Y <= b.Y && a.Z <= b.Z && Vector3.DistanceSquared(a, b) is 36
               select (a, b);
    }

    private static (byte R, byte G, byte B) Hsl(float hue, float saturation, float lightness)
    {
        var chroma = (1 - Math.Abs(2 * lightness - 1)) * saturation;
        var h = (hue % 1 + 1) % 1 * 6;
        var x = chroma * (1 - Math.Abs(h % 2 - 1));
        var (r, g, b) = (int)h switch
        {
            0 => (chroma, x, 0f),
            1 => (x, chroma, 0f),
            2 => (0f, chroma, x),
            3 => (0f, x, chroma),
            4 => (x, 0f, chroma),
            _ => (chroma, 0f, x),
        };
        var m = lightness - chroma / 2;
        return ((byte)((r + m) * 255), (byte)((g + m) * 255), (byte)((b + m) * 255));
    }
}

/// <summary>An RGB image that starts white, with lines and filled triangles.</summary>
internal sealed class Canvas(int width, int height)
{
    public byte[] Rgb { get; } = Enumerable.Repeat((byte)255, width * height * 3).ToArray();

    public void Line(Vector3 from, Vector3 to, (byte R, byte G, byte B) color)
    {
        var steps = (int)Math.Max(Math.Abs(to.X - from.X), Math.Abs(to.Y - from.Y)) + 1;
        for (var i = 0; i <= steps; i++)
        {
            var p = Vector3.Lerp(from, to, (float)i / steps);
            Set((int)p.X, (int)p.Y, color);
        }
    }

    public void FillTriangle(Vector3 a, Vector3 b, Vector3 c, (byte R, byte G, byte B) color)
    {
        var minX = (int)Math.Max(0, Math.Floor(Math.Min(a.X, Math.Min(b.X, c.X))));
        var maxX = (int)Math.Min(width - 1, Math.Ceiling(Math.Max(a.X, Math.Max(b.X, c.X))));
        var minY = (int)Math.Max(0, Math.Floor(Math.Min(a.Y, Math.Min(b.Y, c.Y))));
        var maxY = (int)Math.Min(height - 1, Math.Ceiling(Math.Max(a.Y, Math.Max(b.Y, c.Y))));
        var area = Edge(a, b, c);
        if (area == 0)
        {
            return;
        }
        for (var y = minY; y <= maxY; y++)
        {
            for (var x = minX; x <= maxX; x++)
            {
                var p = new Vector3(x + 0.5f, y + 0.5f, 0);
                // The point is inside when it's on the same side of all three edges.
                var w0 = Edge(b, c, p) / area;
                var w1 = Edge(c, a, p) / area;
                var w2 = Edge(a, b, p) / area;
                if (w0 >= 0 && w1 >= 0 && w2 >= 0)
                {
                    Set(x, y, color);
                }
            }
        }
    }

    private static float Edge(Vector3 a, Vector3 b, Vector3 p) => (b.X - a.X) * (p.Y - a.Y) - (b.Y - a.Y) * (p.X - a.X);

    private void Set(int x, int y, (byte R, byte G, byte B) color)
    {
        if (x < 0 || y < 0 || x >= width || y >= height)
        {
            return;
        }
        var i = (y * width + x) * 3;
        (Rgb[i], Rgb[i + 1], Rgb[i + 2]) = color;
    }
}
