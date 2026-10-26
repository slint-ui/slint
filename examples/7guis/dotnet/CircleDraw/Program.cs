// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using CircleDraw;
using Slint;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        var circles = new ListModel<Circle>();
        window.Model = circles;
        var history = new UndoStack();
        history.Changed += () =>
        {
            window.Undoable = history.CanUndo;
            window.Redoable = history.CanRedo;
        };

        window.OnBackgroundClicked((x, y) =>
        {
            var circle = new Circle { X = x, Y = y, D = 30 };
            var row = circles.Count;
            history.Do(() => circles.Insert(row, circle), () => circles.RemoveAt(row));
        });
        window.OnCircleResized((row, diameter) =>
        {
            var before = circles[row];
            var after = before with { D = diameter };
            history.Do(() => circles[row] = after, () => circles[row] = before);
        });
        window.OnUndoClicked(history.Undo);
        window.OnRedoClicked(history.Redo);

        window.Run();
    }
}

/// <summary>The changes that the user can undo, and the undone ones that they can redo.</summary>
internal sealed class UndoStack
{
    private readonly List<(Action Redo, Action Undo)> _changes = [];
    // The changes before this index are done, the ones after it undone.
    private int _next;

    public event Action? Changed;

    public bool CanUndo => _next > 0;

    public bool CanRedo => _next < _changes.Count;

    /// <summary>Applies a change, and forgets the changes that the user undid.</summary>
    public void Do(Action redo, Action undo)
    {
        redo();
        _changes.RemoveRange(_next, _changes.Count - _next);
        _changes.Add((redo, undo));
        _next++;
        Changed?.Invoke();
    }

    public void Undo()
    {
        _next--;
        _changes[_next].Undo();
        Changed?.Invoke();
    }

    public void Redo()
    {
        _changes[_next].Redo();
        _next++;
        Changed?.Invoke();
    }
}
