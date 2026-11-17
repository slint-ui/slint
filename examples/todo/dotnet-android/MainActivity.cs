// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

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
    protected override byte[]? SlintSaveState() =>
        _window is null || _todos is null ? null : TodoState.Save(_window, _todos);

    protected override void SlintRestoreState(byte[] state)
    {
        if (_window is not null && _todos is not null)
        {
            TodoState.Restore(_window, _todos, state);
        }
    }
}
