// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Diagnostics;
using Launcher;
using Slint;

internal static class Program
{
    /// <summary>A C# port of a demo or example: the directory of its project in the repository, and the project's name.</summary>
    private sealed record Demo(string Directory, string Name, string Title, string Description);

    private static readonly Demo[] Catalog =
    [
        new("demos/printerdemo/dotnet", "PrinterDemo", "Printer Demo", "A fictional user interface for the touch screen of a printer."),
        new("demos/energy-monitor/dotnet", "EnergyMonitor", "Energy Monitor", "A fictional user interface of a device that monitors energy consumption in a building."),
        new("demos/home-automation/dotnet", "HomeAutomation", "Home Automation", "A fictional user interface of a device that automates the control of a home."),
        new("demos/usecases/dotnet", "Usecases", "Usecases", "Different example use cases in one app."),
        new("examples/gallery/dotnet", "Gallery", "Widgets Gallery", "A gallery of the standard widgets that come with Slint."),
        new("examples/todo/dotnet", "Todo", "Todo", "A simple todo list application."),
        new("examples/memory/dotnet", "Memory", "Memory Game", "The memory game from the Slint tutorial."),
        new("examples/slide_puzzle/dotnet", "SlidePuzzle", "Slide Puzzle", "A sliding tile puzzle game."),
        new("examples/carousel/dotnet", "Carousel", "Carousel", "A custom carousel widget that can be controlled by touch, mouse, and keyboard."),
        new("examples/speedometer/dotnet", "Speedometer", "Speedometer", "A dashboard with an animated speedometer."),
    ];

    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = LauncherWindow.Create();
        var launcher = window.Launcher;

        // Only the ports that were built show up, from the build configuration of this launcher.
        var installed = FindRepository() is { } repository
            ? Catalog.Select(demo => (Demo: demo, Path: Built(repository, demo))).Where(entry => entry.Path is not null).ToList()
            : [];
        launcher.Entries = new ListModel<LauncherEntry>(installed.Select(entry => new LauncherEntry { Title = entry.Demo.Title, Description = entry.Demo.Description }));
        if (installed.Count == 0)
        {
            launcher.Status = "Build the C# examples first: dotnet build examples/dotnet.slnx";
        }

        launcher.OnLaunch(async index =>
        {
            if (index < 0 || index >= installed.Count)
            {
                return;
            }
            var (demo, path) = installed[index];
            launcher.Status = "";
            try
            {
                using var process = Process.Start(new ProcessStartInfo("dotnet", [path!]) { WorkingDirectory = Path.GetDirectoryName(path) })!;
                await process.WaitForExitAsync();
                launcher.Status = process.ExitCode == 0 ? "" : $"{demo.Title} exited with {process.ExitCode}";
            }
            catch (Exception e) when (e is System.ComponentModel.Win32Exception or InvalidOperationException)
            {
                launcher.Status = $"Failed to start {demo.Title}: {e.Message}";
            }
        });

        window.Run();
    }

    /// <summary>The root of the repository, which this launcher runs from inside of.</summary>
    private static string? FindRepository()
    {
        for (var dir = new DirectoryInfo(AppContext.BaseDirectory); dir is not null; dir = dir.Parent)
        {
            if (File.Exists(Path.Combine(dir.FullName, "examples", "dotnet.slnx")))
            {
                return dir.FullName;
            }
        }
        return null;
    }

    /// <summary>The built assembly of <paramref name="demo"/>, next to the launcher's in the same configuration, if it exists.</summary>
    private static string? Built(string repository, Demo demo)
    {
        // The launcher runs from <project>/bin/<configuration>/<framework>/.
        var output = new DirectoryInfo(AppContext.BaseDirectory.TrimEnd(Path.DirectorySeparatorChar));
        var path = Path.Combine(repository, demo.Directory, "bin", output.Parent!.Name, output.Name, demo.Name + ".dll");
        return File.Exists(path) ? path : null;
    }
}
