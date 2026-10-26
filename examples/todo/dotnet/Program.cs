// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Slint;
using Todo;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        var todos = new ListModel<TodoItem>(
        [
            new() { Title = "Implement the .slint file", Checked = true },
            new() { Title = "Do the Rust part", Checked = true },
            new() { Title = "Make the C++ code" },
            new() { Title = "Write some JavaScript code" },
            new() { Title = "Write some C# code", Checked = true },
            new() { Title = "Test the application" },
            new() { Title = "Ship to customer" },
            new() { Title = "???" },
            new() { Title = "Profit" },
        ]);

        using var window = MainWindow.Create();

        window.OnTodoAdded(title => todos.Add(new TodoItem { Title = title }));
        window.OnRemoveDone(() =>
        {
            for (var i = todos.Count - 1; i >= 0; i--)
            {
                if (todos[i].Checked)
                {
                    todos.RemoveAt(i);
                }
            }
        });

        // Closing with unfinished items asks first.
        window.OnPopupConfirmed(window.Hide);
        window.Window.OnCloseRequested(() =>
        {
            if (todos.Any(todo => !todo.Checked))
            {
                window.InvokeShowConfirmPopup();
                return CloseRequestResponse.KeepWindowShown;
            }
            return CloseRequestResponse.HideWindow;
        });

        window.OnApplySortingAndFiltering(() =>
        {
            Model<TodoItem> shown = todos;
            if (window.HideDoneItems)
            {
                shown = shown.Filter(todo => !todo.Checked);
            }
            if (window.IsSortByName)
            {
                shown = shown.Sort((a, b) => string.Compare(a.Title, b.Title, StringComparison.CurrentCultureIgnoreCase));
            }
            window.TodoModel = shown;
        });

        window.ShowHeader = true;
        window.TodoModel = todos;
        window.Run();
    }
}
