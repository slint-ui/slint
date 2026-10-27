// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Text.Json;
using System.Text.Json.Serialization;
using Slint;
using Slint.Android;

namespace Todo;

[Activity(Label = "Slint Todo", MainLauncher = true, Exported = true, ConfigurationChanges = SlintActivity.ConfigurationChanges)]
[MetaData(SlintActivity.LibraryNameKey, Value = SlintActivity.LibraryName)]
public class MainActivity : SlintActivity
{
    private MainWindow? _window;
    private ListModel<TodoItem>? _todos;

    protected override void SlintMain()
    {
        using var window = MainWindow.Create();
        _window = window;
        _todos = TodoLogic.Connect(window);
        window.ShowHeader = true;
        window.Run();
    }

    // Keeps the list, like the Rust version, when Android ends the process in the background.
    protected override byte[]? SlintSaveState() => _window is null || _todos is null
        ? null
        : JsonSerializer.SerializeToUtf8Bytes(
            new SavedTodos([.. _todos.Select(t => new SavedTodo(t.Title, t.Checked))], _window.IsSortByName, _window.HideDoneItems),
            TodoJson.Default.SavedTodos);

    protected override void SlintRestoreState(byte[] state)
    {
        if (_window is null || _todos is null || JsonSerializer.Deserialize(state, TodoJson.Default.SavedTodos) is not { } saved)
        {
            return;
        }
        _todos.Clear();
        _todos.AddRange(saved.Items.Select(t => new TodoItem { Title = t.Title, Checked = t.Checked }));
        _window.IsSortByName = saved.SortByName;
        _window.HideDoneItems = saved.HideDone;
        _window.InvokeApplySortingAndFiltering();
    }
}

internal sealed record SavedTodo(string Title, bool Checked);

internal sealed record SavedTodos(List<SavedTodo> Items, bool SortByName, bool HideDone);

// Generated serialization, which works with the trimming of Release builds.
[JsonSerializable(typeof(SavedTodos))]
internal sealed partial class TodoJson : JsonSerializerContext;
