// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

// Starts each built project of a solution with the testing backend,
// and fails for each one that exits before the time is up, such as on an exception at startup.
// Usage: dotnet run examples/dotnet-smoke-test.cs -- <solution> [seconds]

using System.Diagnostics;
using System.Xml.Linq;

if (args.Length is < 1 or > 2)
{
    Console.Error.WriteLine("Usage: dotnet run dotnet-smoke-test.cs -- <solution> [seconds]");
    return 2;
}
var solution = Path.GetFullPath(args[0]);
var seconds = args.Length > 1 ? int.Parse(args[1], System.Globalization.CultureInfo.InvariantCulture) : 5;

// Projects that need arguments to show a window.
var arguments = new Dictionary<string, string[]>
{
    ["DotnetViewer"] = ["../memory/memory.slint"],
};

var projects = XDocument.Load(solution).Descendants("Project")
    .Select(p => Path.GetFullPath(Path.Combine(Path.GetDirectoryName(solution)!, p.Attribute("Path")!.Value)))
    .ToList();
var results = await Task.WhenAll(projects.Select(Run));
foreach (var (name, failure) in results)
{
    Console.WriteLine(failure is null ? $"ok      {name}" : $"FAILED  {name}: {failure}");
}
var failed = results.Count(r => r.Failure is not null);
Console.WriteLine($"{projects.Count - failed} of {projects.Count} projects kept running for {seconds} seconds.");
return failed == 0 ? 0 : 1;

async Task<(string Name, string? Failure)> Run(string project)
{
    var name = Path.GetFileNameWithoutExtension(project);
    var directory = Path.GetDirectoryName(project)!;
    var assembly = Path.Combine(directory, "bin", "Debug", "net10.0", name + ".dll");
    if (!File.Exists(assembly))
    {
        return (name, $"{assembly} not found; build the solution first");
    }

    var start = new ProcessStartInfo("dotnet")
    {
        WorkingDirectory = directory,
        RedirectStandardOutput = true,
        RedirectStandardError = true,
    };
    start.ArgumentList.Add(assembly);
    foreach (var argument in arguments.GetValueOrDefault(name, []))
    {
        start.ArgumentList.Add(argument);
    }
    start.Environment["SLINT_BACKEND"] = "testing";

    using var process = Process.Start(start)!;
    var output = process.StandardOutput.ReadToEndAsync();
    var error = process.StandardError.ReadToEndAsync();
    var exited = process.WaitForExitAsync();
    if (await Task.WhenAny(exited, Task.Delay(TimeSpan.FromSeconds(seconds))) != exited)
    {
        process.Kill(entireProcessTree: true);
        await exited;
        return (name, null);
    }
    return (name, $"exited with code {process.ExitCode}\n{await output}{await error}");
}
