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
  The rows live in a small `Model<Lol>` that tells Slint once per frame which rows changed,
  instead of once per lol, as a canvas draws once per frame.
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
Release builds with .NET 10 on a Motorola edge 60 neo with Android 16,
next to the original apps from the `main` and `canvas` branches of LOLs per second,
and a [Flutter version](https://github.com/yurkinh/lols/tree/slint/src/flutter) with the same modes.
To run the same comparison on your own phone, use the
[`slint` branch of the lols fork](https://github.com/yurkinh/lols/tree/slint#comparing-on-your-own-phone),
whose `scripts/compare-android.sh` builds every version, runs each on the connected device, and prints the rates:

| App | One label per lol, 5,000 | Canvas, 100,000 |
| --- | --- | --- |
| Java, `TextView` | 625 LOL/s | |
| .NET for Android, `TextView` | 587 LOL/s | 161,656 LOL/s, one `View` drawing on a `Canvas`, counted when queued |
| .NET MAUI, `Label` | 271 LOL/s, with 2 ms sleeps | |
| Flutter, `Text` widgets and a `CustomPainter` | 880 LOL/s, `elements` | 158,000 to 170,000 LOL/s, `canvas` |
| Slint for .NET | 821 LOL/s, `elements` | 170,000 to 204,000 LOL/s, `canvas` |

The originals post each label to the UI thread, while the Flutter version and this port apply a frame's worth of lols at once.
In `canvas`, both store each lol and update the screen once per frame.
Notifying Slint of every lol instead halves this port's rate, to about 98,000 LOL/s.
The original canvas version counts a lol when it's queued, and this port when the UI thread has applied it.

The one sleep per lol limits every `elements` rate, so its 821 is close to how long a 1 ms sleep takes on that phone.
As the original's README says about its `canvas` branch, canvas numbers measure how fast lols reach the renderer,
which still draws the latest 500 once per frame.
