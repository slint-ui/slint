// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Slint.Android;

namespace Todo;

[Activity(Label = "Slint Todo", MainLauncher = true, Exported = true, ConfigurationChanges = SlintActivity.ConfigurationChanges)]
[MetaData(SlintActivity.LibraryNameKey, Value = SlintActivity.LibraryName)]
public class MainActivity : SlintActivity
{
    protected override void SlintMain()
    {
        using var window = MainWindow.Create();
        TodoLogic.Connect(window);
        window.ShowHeader = true;
        window.Run();
    }
}
