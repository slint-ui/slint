// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Slint;
using SlidePuzzle;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        using var game = new Game(window);
        window.Run();
    }
}

/// <summary>The 15-puzzle: 15 pieces and a hole on a 4×4 grid.</summary>
internal sealed class Game : IDisposable
{
    private const int PieceCount = 15;

    private readonly MainWindow _window;
    private readonly ListModel<Piece> _pieces = new(Enumerable.Repeat(new Piece(), PieceCount));
    private readonly Slint.Timer _autoPlay = new();
    private readonly Slint.Timer _kick = new();
    // The speed of the spring animation of each piece that the user clicked but that can't move.
    private readonly (float X, float Y)[] _kickSpeed = new (float, float)[PieceCount];
    // The piece at each cell of the grid, or -1 for the hole.
    private int[] _positions = [];
    private bool _finished;

    public Game(MainWindow window)
    {
        _window = window;
        window.Pieces = _pieces;
        window.OnPieceClicked(piece =>
        {
            StopAutoPlay();
            if (!_finished && !Click(piece))
            {
                _kick.Start(TimerMode.Repeated, TimeSpan.FromMilliseconds(16), Kick);
            }
        });
        window.OnReset(() =>
        {
            StopAutoPlay();
            Randomize();
        });
        window.OnEnableAutoMode(enabled =>
        {
            if (enabled)
            {
                _autoPlay.Start(TimerMode.Repeated, TimeSpan.FromMilliseconds(200), RandomMove);
            }
            else
            {
                _autoPlay.Stop();
            }
        });
        Randomize();
    }

    public void Dispose()
    {
        _autoPlay.Dispose();
        _kick.Dispose();
    }

    private void StopAutoPlay()
    {
        _autoPlay.Stop();
        _window.AutoPlay = false;
    }

    private void Randomize()
    {
        _positions = Shuffle();
        for (var cell = 0; cell < _positions.Length; cell++)
        {
            Place(_positions[cell], cell);
        }
        _window.Moves = 0;
        UpdateTilesLeft();
    }

    private static int[] Shuffle()
    {
        int[] cells = [-1, .. Enumerable.Range(0, PieceCount)];
        do
        {
            Random.Shared.Shuffle(cells);
        }
        while (!IsSolvable(cells));
        return cells;
    }

    /// <summary>Half of all arrangements can't be solved, which the parity of the inversions and the row of the hole tells.</summary>
    private static bool IsSolvable(int[] cells)
    {
        var inversions = 0;
        for (var i = 0; i < cells.Length - 1; i++)
        {
            for (var j = i + 1; j < cells.Length; j++)
            {
                if (cells[j] >= 0 && cells[j] < cells[i])
                {
                    inversions++;
                }
            }
        }
        var holeRow = Array.IndexOf(cells, -1) / 4;
        return inversions % 2 != holeRow % 2;
    }

    private void Place(int piece, int cell)
    {
        if (piece >= 0)
        {
            _pieces[piece] = new Piece { PosX = cell / 4, PosY = cell % 4 };
        }
    }

    private void UpdateTilesLeft()
    {
        var left = PieceCount - _positions.Where((piece, cell) => piece == cell).Count();
        _window.TilesLeft = left;
        _finished = left == 0;
    }

    /// <summary>Slides the clicked piece, and the ones between it and the hole, toward the hole.</summary>
    /// <returns>Whether the piece moved, since only a piece in the row or column of the hole can.</returns>
    private bool Click(int piece)
    {
        var current = _pieces[piece];
        var cell = current.PosX * 4 + current.PosY;
        var hole = Array.IndexOf(_positions, -1);
        var direction = cell > hole ? -1 : 1;
        if (hole % 4 == current.PosY)
        {
            Slide(cell, direction * 4);
        }
        else if (hole / 4 == current.PosX)
        {
            Slide(cell, direction);
        }
        else
        {
            _kickSpeed[piece] = (hole % 4 > current.PosY ? 10 : -10, hole / 4 > current.PosX ? 10 : -10);
            return false;
        }
        UpdateTilesLeft();
        _window.Moves++;
        return true;
    }

    private void Slide(int cell, int step)
    {
        var swap = cell;
        while (_positions[cell] != -1)
        {
            swap += step;
            (_positions[cell], _positions[swap]) = (_positions[swap], _positions[cell]);
            Place(_positions[swap], swap);
        }
    }

    private void RandomMove()
    {
        var hole = Array.IndexOf(_positions, -1);
        int cell;
        do
        {
            cell = Random.Shared.Next(16);
        }
        while (cell == hole || (hole % 4 != cell % 4 && hole / 4 != cell / 4));
        Click(_positions[cell]);
    }

    /// <summary>Shakes the pieces that couldn't move, until their springs settle.</summary>
    private void Kick()
    {
        var animating = false;
        for (var i = 0; i < PieceCount; i++)
        {
            var previous = _kickSpeed[i];
            var piece = _pieces[i];
            var (offsetX, speedX) = Spring(piece.OffsetX, previous.X);
            var (offsetY, speedY) = Spring(piece.OffsetY, previous.Y);
            _kickSpeed[i] = (speedX, speedY);
            if (speedX != 0 || speedY != 0 || previous.X != 0 || previous.Y != 0)
            {
                _pieces[i] = piece with { OffsetX = offsetX, OffsetY = offsetY };
                animating = true;
            }
        }
        if (!animating)
        {
            _kick.Stop();
        }
    }

    private static (float Offset, float Speed) Spring(float offset, float speed)
    {
        const float Stiffness = 0.3f;
        const float Damping = 0.7f;
        const float Epsilon = 0.3f;
        speed = (speed - offset * Stiffness) * Damping;
        if (speed != 0 || offset != 0)
        {
            offset += speed;
            if (Math.Abs(speed) < Epsilon && Math.Abs(offset) < Epsilon)
            {
                (offset, speed) = (0, 0);
            }
        }
        return (offset, speed);
    }
}
