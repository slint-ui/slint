// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Memory;
using Slint;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();

        var initialTiles = window.MemoryTiles.ToList();
        var tiles = new ListModel<TileData>(initialTiles.Concat(initialTiles).OrderBy(_ => Random.Shared.Next()));
        window.MemoryTiles = tiles;

        window.OnCheckIfPairSolved(() =>
        {
            var flipped = tiles
                .Select((tile, index) => (Tile: tile, Index: index))
                .Where(t => t.Tile.ImageVisible && !t.Tile.Solved)
                .ToList();
            if (flipped.Count != 2)
            {
                return;
            }

            var (first, second) = (flipped[0], flipped[1]);
            if (first.Tile.Image.Path == second.Tile.Image.Path)
            {
                tiles[first.Index] = first.Tile with { Solved = true };
                tiles[second.Index] = second.Tile with { Solved = true };
            }
            else
            {
                window.DisableTiles = true;
                Slint.Timer.SingleShot(TimeSpan.FromSeconds(1), () =>
                {
                    window.DisableTiles = false;
                    tiles[first.Index] = first.Tile with { ImageVisible = false };
                    tiles[second.Index] = second.Tile with { ImageVisible = false };
                });
            }
        });

        window.Run();
    }
}
