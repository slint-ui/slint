// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Collections.Concurrent;
using System.Diagnostics;
using System.Globalization;
using Slint;

namespace Lols;

/// <summary>
/// The test of https://github.com/jonathanpeppers/lols, which the desktop and the Android versions share.
/// <c>canvas</c> follows LolsView of its canvas branch, and <c>elements</c> the TextView version of its main branch.
/// </summary>
internal sealed class LolsBenchmark : IDisposable
{
    public static readonly string[] Modes = ["canvas", "elements"];

    private const int Max = 500;

    private readonly Slint.Timer _drain = new();
    private readonly Slint.Timer _update = new();
    private readonly Stopwatch _stopwatch = Stopwatch.StartNew();
    private int _count;

    private LolsBenchmark(MainWindow window, string mode, Action<string> report)
    {
        // canvas queues 100000 lols and sleeps every 256; elements adds 5000 and sleeps after each.
        var total = mode == "canvas" ? 100_000 : 5_000;
        var model = new ListModel<Lol>();
        window.Lols = model;

        // Slint objects belong to the Slint thread, so the producer only fills the queue.
        var queue = new ConcurrentQueue<Lol>();
        _ = Task.Factory.StartNew(() =>
        {
            var random = Random.Shared;
            for (var produced = 1; produced <= total; produced++)
            {
                queue.Enqueue(new Lol
                {
                    X = random.NextSingle(),
                    Y = random.NextSingle(),
                    Color = Color.FromArgb(0xFF000000 | (uint)random.Next(0xFFFFFF)),
                    Rotation = random.NextSingle() * 360,
                });
                if (mode == "elements" || produced % 256 == 0)
                {
                    Thread.Sleep(1);
                }
            }
        }, TaskCreationOptions.LongRunning);

        var next = 0;
        // Applies the queued lols about once per frame, like Invalidate() on Android.
        _drain.Start(TimerMode.Repeated, TimeSpan.FromMilliseconds(16), () =>
        {
            while (_count < total && queue.TryDequeue(out var lol))
            {
                if (mode == "elements")
                {
                    if (model.Count >= Max)
                    {
                        model.RemoveAt(0);
                    }
                    model.Add(lol);
                }
                else if (model.Count < Max)
                {
                    model.Add(lol);
                }
                else
                {
                    // A fixed set of rows: Slint only updates properties, and creates no elements.
                    model[next] = lol;
                    next = (next + 1) % Max;
                }
                _count++;
            }
            if (_count == total && _stopwatch.IsRunning)
            {
                _stopwatch.Stop();
                report(string.Create(CultureInfo.InvariantCulture,
                    $"{mode}: {Rate()} after {_count} lols in {_stopwatch.Elapsed.TotalSeconds:0.00} s"));
            }
        });
        _update.Start(TimerMode.Repeated, TimeSpan.FromMilliseconds(500), () => window.Rate = Rate());
    }

    /// <summary>Starts the test in <paramref name="window"/>, which reports the result through <paramref name="report"/>.</summary>
    public static LolsBenchmark Start(MainWindow window, string mode, Action<string> report) => new(window, mode, report);

    public void Dispose()
    {
        _drain.Dispose();
        _update.Dispose();
    }

    private string Rate() => "LOL/s: " + (_count / _stopwatch.Elapsed.TotalSeconds).ToString("0.00", CultureInfo.InvariantCulture);
}
