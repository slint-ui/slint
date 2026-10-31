// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Android.Util;
using Slint.Android;

namespace Lols;

// Pick the mode with: adb shell am start -n dev.slint.dotnet.lols/lols.MainActivity --es mode elements
[Activity(Name = "lols.MainActivity", Label = "LOLs per second", MainLauncher = true, Exported = true,
    ConfigurationChanges = SlintActivity.ConfigurationChanges)]
[MetaData(SlintActivity.LibraryNameKey, Value = SlintActivity.LibraryName)]
public class MainActivity : SlintActivity
{
    protected override void SlintMain()
    {
        var mode = Intent?.GetStringExtra("mode") ?? LolsBenchmark.Modes[0];
        if (!LolsBenchmark.Modes.Contains(mode))
        {
            throw new ArgumentException($"Unknown mode {mode}, expected one of {string.Join(", ", LolsBenchmark.Modes)}");
        }

        using var window = MainWindow.Create();
        using var benchmark = LolsBenchmark.Start(window, mode, result => Log.Info("Lols", result));
        window.Run();
    }
}
