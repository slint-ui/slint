// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Slint;

internal static class Program
{
    private const string Usage = """
        Shows a .slint file, or a project directory with a slint-project.json, without generated code.

        Usage: dotnet-viewer <path> [--component <name>] [--style <style>] [--live]

          --component  the component to show; by default the last exported one
          --style      the widget style, such as fluent, material, or cupertino
          --live       reload the component when the file or a file it imports changes
        """;

    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static int Main(string[] args)
    {
        string? path = null, component = null, style = null;
        var live = false;
        for (var i = 0; i < args.Length; i++)
        {
            switch (args[i])
            {
                case "--component" when i + 1 < args.Length:
                    component = args[++i];
                    break;
                case "--style" when i + 1 < args.Length:
                    style = args[++i];
                    break;
                case "--live":
                    live = true;
                    break;
                case var arg when !arg.StartsWith('-') && path is null:
                    path = arg;
                    break;
                default:
                    Console.Error.WriteLine(Usage);
                    return 2;
            }
        }
        if (path is null)
        {
            Console.Error.WriteLine(Usage);
            return 2;
        }

        using var compiler = new Compiler();
        if (style is not null)
        {
            compiler.Style = style;
        }
        using var result = compiler.BuildFromPath(path);
        foreach (var diagnostic in result.Diagnostics)
        {
            Console.Error.WriteLine(diagnostic);
        }
        if (result.HasErrors)
        {
            return 1;
        }
        component ??= result.ComponentNames.LastOrDefault();
        if (component is null || !result.ComponentNames.Contains(component))
        {
            Console.Error.WriteLine($"{path} has no component '{component}'; it has: {string.Join(", ", result.ComponentNames)}");
            return 1;
        }

        using var instance = live
            ? ComponentInstance.CreateLive(path, component, style)
            : result.Component(component)!.Create();
        instance.Reloaded += () => Console.WriteLine($"Reloaded {path}");
        instance.Run();
        return 0;
    }
}
