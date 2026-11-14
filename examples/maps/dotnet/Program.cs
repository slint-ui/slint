// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Diagnostics;
using Maps;
using Slint;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainUI.Create();
        var world = new World(window);

        window.OnFlicked((x, y) =>
        {
            world.Offset = (-x, -y);
            world.Refresh();
        });
        window.OnZoomChanged(zoom => world.Zoom((int)MathF.Round(zoom), window.VisibleWidth / 2, window.VisibleHeight / 2));
        window.OnZoomIn((x, y) => world.Zoom(world.ZoomLevel + 1, x, y));
        window.OnZoomOut((x, y) => world.Zoom(world.ZoomLevel - 1, x, y));
        window.OnLinkClicked(() => Process.Start(new ProcessStartInfo("https://www.openstreetmap.org/copyright") { UseShellExecute = true }));

        // The window has its size once the event loop runs.
        EventLoop.InvokeFromEventLoop(world.Refresh);
        window.Run();
    }
}

/// <summary>The tiles of OpenStreetMap that the window shows, which it downloads as the user pans and zooms.</summary>
internal sealed class World(MainUI window)
{
    private const int TileSize = 256;
    // Tiles this far outside the view stay loaded, so that panning back doesn't download them again.
    private const int KeepCached = 10;

    private static readonly HttpClient Http = CreateClient();
    private static readonly string TilesUrl = Environment.GetEnvironmentVariable("OSM_TILES_URL") ?? "https://tile.openstreetmap.org";

    private readonly Dictionary<(int Z, int X, int Y), Image> _loaded = [];
    private readonly HashSet<(int Z, int X, int Y)> _loading = [];
    private readonly ListModel<Tile> _tiles = [];
    private bool _modelSet;

    public int ZoomLevel { get; private set; } = 1;

    /// <summary>The position of the view in the world, in pixels at the current zoom level.</summary>
    public (float X, float Y) Offset { get; set; }

    /// <summary>Zooms around the point of the view at (<paramref name="x"/>, <paramref name="y"/>).</summary>
    public void Zoom(int zoomLevel, float x, float y)
    {
        zoomLevel = Math.Clamp(zoomLevel, 1, 19);
        if (zoomLevel != ZoomLevel)
        {
            var scale = MathF.Pow(2, zoomLevel - ZoomLevel);
            Offset = ((Offset.X + x) * scale - x, (Offset.Y + y) * scale - y);
            ZoomLevel = zoomLevel;
            _loaded.Clear();
            _loading.Clear();
        }
        window.Zoom = ZoomLevel;
        var worldSize = TileSize * (1 << ZoomLevel);
        window.SetViewport(-Offset.X, -Offset.Y, worldSize, worldSize);
        Refresh();
    }

    /// <summary>Starts to download the visible tiles that aren't loaded, and shows the loaded ones.</summary>
    public void Refresh()
    {
        var count = 1 << ZoomLevel;
        var minX = (int)Math.Floor(Offset.X / TileSize);
        var minY = (int)Math.Floor(Offset.Y / TileSize);
        var maxX = Math.Min((int)Math.Ceiling((Offset.X + window.VisibleWidth) / TileSize) + 1, count);
        var maxY = Math.Min((int)Math.Ceiling((Offset.Y + window.VisibleHeight) / TileSize) + 1, count);

        bool Keep((int Z, int X, int Y) tile) => tile.Z == ZoomLevel
            && tile.X > minX - KeepCached && tile.X < maxX + KeepCached
            && tile.Y > minY - KeepCached && tile.Y < maxY + KeepCached;
        foreach (var tile in _loaded.Keys.Where(tile => !Keep(tile)).ToList())
        {
            _loaded.Remove(tile);
        }
        _loading.RemoveWhere(tile => !Keep(tile));

        for (var x = Math.Max(minX, 0); x < maxX; x++)
        {
            for (var y = Math.Max(minY, 0); y < maxY; y++)
            {
                var tile = (ZoomLevel, x, y);
                if (!_loaded.ContainsKey(tile) && _loading.Add(tile))
                {
                    _ = LoadAsync(tile);
                }
            }
        }
        ShowLoaded();
    }

    private async Task LoadAsync((int Z, int X, int Y) tile)
    {
        var url = $"{TilesUrl}/{tile.Z}/{tile.X}/{tile.Y}.png";
        try
        {
            var image = Image.LoadFromData(await Http.GetByteArrayAsync(url), "png");
            // The await continues on the Slint thread; the tile may have scrolled out of view meanwhile.
            if (_loading.Remove(tile))
            {
                _loaded[tile] = image;
                ShowLoaded();
            }
        }
        catch (Exception e) when (e is HttpRequestException or SlintException)
        {
            _loading.Remove(tile);
            Console.Error.WriteLine($"Error loading {url}: {e.Message}");
        }
    }

    private void ShowLoaded()
    {
        var tiles = _loaded.Select(entry => new Tile { X = entry.Key.X * TileSize, Y = entry.Key.Y * TileSize, Tile_ = entry.Value }).ToList();
        // Updating the rows in place keeps the tiles that stay from being created again.
        for (var i = 0; i < tiles.Count; i++)
        {
            if (i < _tiles.Count)
            {
                _tiles[i] = tiles[i];
            }
            else
            {
                _tiles.Add(tiles[i]);
            }
        }
        while (_tiles.Count > tiles.Count)
        {
            _tiles.RemoveAt(_tiles.Count - 1);
        }
        if (!_modelSet)
        {
            window.Tiles = _tiles;
            _modelSet = true;
        }
    }

    /// <summary>OpenStreetMap asks clients to identify themselves.</summary>
    private static HttpClient CreateClient()
    {
        var client = new HttpClient();
        client.DefaultRequestHeaders.UserAgent.ParseAdd("Slint-Maps-example/1.0");
        return client;
    }
}
