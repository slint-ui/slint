### `lols`

A port of [LOLs per second](https://github.com/jonathanpeppers/lols), a performance test of .NET UI frameworks, to Slint for .NET.
It draws "lol?" in random colors, positions, and rotations, and shows how many it adds per second.

| `.slint` Design | C# Source | Android |
| --- | --- | --- |
| [`lols.slint`](./ui/lols.slint) | [`LolsBenchmark.cs`](./dotnet/LolsBenchmark.cs), [`Program.cs`](./dotnet/Program.cs) | [`MainActivity.cs`](./dotnet-android/MainActivity.cs) |

It has two modes:

- `canvas`, the default, follows `LolsView` on the original's `canvas` branch:
  a background thread queues 100,000 lols and sleeps 1 ms after every 256.
  The window shows the latest 500 in a fixed set of rows, so Slint updates properties and creates no elements.
- `elements` follows the `TextView` version on the original's `main` branch:
  a background thread queues 5,000 lols and sleeps 1 ms after each.
  Slint adds a `Text` for each lol and removes the oldest one at 500.

Run it on the desktop:

```sh
dotnet run --project examples/lols/dotnet -- elements
```

Run it on Android, see [Android](../../docs/development/dotnet-bindings.md#android) for the native library:

```sh
dotnet build examples/lols/dotnet-android -c Release -p:SlintAndroid=true
adb install -r examples/lols/dotnet-android/bin/Release/net10.0-android/dev.slint.dotnet.lols-Signed.apk
adb shell am start -n dev.slint.dotnet.lols/lols.MainActivity --es mode elements
adb logcat -s Lols
```

The app prints the final rate when all lols are in.
Both rates are mostly limited by the `Thread.Sleep` calls of the original:
256 lols per sleep in `canvas`, and one lol per sleep in `elements`.
As the original's README says about its `canvas` branch, such numbers mostly count objects put in a queue.
