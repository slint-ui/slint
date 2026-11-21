// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Text.Json;
using System.Text.Json.Serialization;
using Slint;

namespace Todo;

/// <summary>
/// The list and the filters as bytes, which the Android and iOS versions keep
/// for when the system ends the app in the background, like the Rust version.
/// </summary>
internal static class TodoState
{
    public static byte[] Save(MainWindow window, ListModel<TodoItem> todos) =>
        JsonSerializer.SerializeToUtf8Bytes(
            new SavedTodos([.. todos.Select(t => new SavedTodo(t.Title, t.Checked))], window.IsSortByName, window.HideDoneItems),
            TodoJson.Default.SavedTodos);

    public static void Restore(MainWindow window, ListModel<TodoItem> todos, byte[] state)
    {
        if (JsonSerializer.Deserialize(state, TodoJson.Default.SavedTodos) is not { } saved)
        {
            return;
        }
        todos.Clear();
        todos.AddRange(saved.Items.Select(t => new TodoItem { Title = t.Title, Checked = t.Checked }));
        window.IsSortByName = saved.SortByName;
        window.HideDoneItems = saved.HideDone;
        window.InvokeApplySortingAndFiltering();
    }
}

internal sealed record SavedTodo(string Title, bool Checked);

internal sealed record SavedTodos(List<SavedTodo> Items, bool SortByName, bool HideDone);

// Generated serialization, which works with the trimming of Release builds.
[JsonSerializable(typeof(SavedTodos))]
internal sealed partial class TodoJson : JsonSerializerContext;
