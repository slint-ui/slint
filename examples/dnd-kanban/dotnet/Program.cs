// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using DndKanban;
using Slint;

internal static class Program
{
    /// <summary>What a drag carries, so that a drop knows the card and where to remove it from on a move.</summary>
    private sealed record DragPayload(TaskData Task, int SourceColumn, int SourceIndex);

    private static readonly EnumValue None = new("DragAction", "none");
    private static readonly EnumValue Copy = new("DragAction", "copy");
    private static readonly EnumValue Move = new("DragAction", "move");

    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        ListModel<TaskData>[] columns =
        [
            new([new() { Title = "Write release notes" }, new() { Title = "Reply to mailing list" }, new() { Title = "Triage open issues" }]),
            new([new() { Title = "Polish drag-and-drop example" }, new() { Title = "Review kanban PR" }]),
            new([new() { Title = "Set up project skeleton" }]),
        ];

        using var window = MainWindow.Create();
        window.Todo = columns[0];
        window.Doing = columns[1];
        window.Done = columns[2];

        window.Api.OnMakeData((task, sourceColumn, sourceIndex) =>
            new DataTransfer { UserData = new DragPayload(task, sourceColumn, sourceIndex) });

        // A card of this window moves or copies as proposed; text from elsewhere becomes a new card.
        window.Api.OnCanDrop((dropEvent, _, _) =>
        {
            var data = (DataTransfer)dropEvent["data"]!;
            if (data.UserData is DragPayload)
            {
                return (EnumValue)dropEvent["proposed-action"]!;
            }
            return data.HasPlainText ? Copy : None;
        });

        window.Api.OnDropped((dropEvent, targetColumn, targetIndex) =>
        {
            if (targetColumn < 0 || targetColumn >= columns.Length)
            {
                return;
            }
            var target = columns[targetColumn];
            var data = (DataTransfer)dropEvent["data"]!;
            if (data.UserData is not DragPayload payload)
            {
                if (data.PlainText is { } text)
                {
                    target.Insert(targetIndex, new TaskData { Title = text });
                }
                return;
            }
            if (!Equals(dropEvent["proposed-action"], Move))
            {
                target.Insert(targetIndex, payload.Task);
                return;
            }
            if (payload.SourceColumn == targetColumn)
            {
                // Dropping a card right before or after itself leaves it in place.
                if (targetIndex == payload.SourceIndex || targetIndex == payload.SourceIndex + 1)
                {
                    return;
                }
                columns[payload.SourceColumn].RemoveAt(payload.SourceIndex);
                target.Insert(targetIndex > payload.SourceIndex ? targetIndex - 1 : targetIndex, payload.Task);
            }
            else
            {
                columns[payload.SourceColumn].RemoveAt(payload.SourceIndex);
                target.Insert(targetIndex, payload.Task);
            }
        });

        window.Run();
    }
}
