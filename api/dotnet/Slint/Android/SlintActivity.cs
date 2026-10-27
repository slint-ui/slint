// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Runtime.InteropServices;
using Android.Content.Res;
using Android.OS;
using Android.Util;
using Slint.Generated;
using Slint.Interop;

namespace Slint.Android;

/// <summary>
/// The activity of a Slint application on Android.
/// Derive the main activity from it, and create and run the components in <see cref="SlintMain"/>:
/// <code>
/// [Activity(MainLauncher = true, ConfigurationChanges = SlintActivity.ConfigurationChanges)]
/// [MetaData(SlintActivity.LibraryNameKey, Value = SlintActivity.LibraryName)]
/// public class MainActivity : SlintActivity
/// {
///     protected override void SlintMain()
///     {
///         using var app = AppWindow.Create();
///         app.Run();
///     }
/// }
/// </code>
/// </summary>
/// <remarks>
/// Slint draws into the window of a <c>NativeActivity</c> and runs its event loop on a thread of its own,
/// which becomes the Slint thread: <see cref="SlintMain"/> runs there, and so does every callback.
/// Slint's objects belong to that thread, so the application runs one activity per process:
/// the activity handles the configuration changes in <see cref="ConfigurationChanges"/> itself,
/// and ends the process when Android destroys it, such as after the Back button.
/// The next launch starts a new process.
/// <para>
/// To keep the app's state when Android ends the process in the background, such as to free memory,
/// return it from <see cref="SlintSaveState"/>, and take it back in <see cref="SlintRestoreState"/>
/// when the user returns to the app.
/// <see cref="SlintPause"/>, <see cref="SlintResume"/>, and <see cref="SlintLowMemory"/> report the other activity events.
/// Unlike <c>OnPause</c> and the other methods of <c>Activity</c>, they all run on the Slint thread.
/// </para>
/// <para>
/// The build puts the <c>.slint</c> files into the APK's assets.
/// Before <see cref="SlintMain"/>, the activity extracts them to <c>slint</c> in its files directory,
/// where the generated code loads them from.
/// </para>
/// </remarks>
public abstract unsafe class SlintActivity : global::Android.App.NativeActivity
{
    /// <summary>The <c>meta-data</c> name that tells <c>NativeActivity</c> which library to load.</summary>
    public const string LibraryNameKey = "android.app.lib_name";

    /// <summary>The value of <see cref="LibraryNameKey"/>: the native library of Slint.</summary>
    public const string LibraryName = NativeMethods.Library;

    /// <summary>The configuration changes that the activity handles without being recreated.</summary>
    public const global::Android.Content.PM.ConfigChanges ConfigurationChanges =
        global::Android.Content.PM.ConfigChanges.Orientation | global::Android.Content.PM.ConfigChanges.ScreenSize
        | global::Android.Content.PM.ConfigChanges.ScreenLayout | global::Android.Content.PM.ConfigChanges.SmallestScreenSize
        | global::Android.Content.PM.ConfigChanges.KeyboardHidden | global::Android.Content.PM.ConfigChanges.Keyboard
        | global::Android.Content.PM.ConfigChanges.UiMode | global::Android.Content.PM.ConfigChanges.Density;

    private const string LogTag = "Slint";
    private const string AssetDirectory = "slint";

    private static SlintActivity? s_current;

    /// <summary>Creates the components and runs the event loop, on the Slint thread.</summary>
    protected abstract void SlintMain();

    /// <summary>
    /// Returns the state to restore if Android ends the process in the background, or <see langword="null"/> for none.
    /// Android waits for it, so keep it quick and small, such as a few kilobytes.
    /// </summary>
    protected virtual byte[]? SlintSaveState() => null;

    /// <summary>
    /// Restores the state that <see cref="SlintSaveState"/> returned before Android ended the previous process.
    /// It runs once, in the event loop that <see cref="SlintMain"/> starts, so after the components exist.
    /// </summary>
    protected virtual void SlintRestoreState(byte[] state)
    {
    }

    /// <summary>The activity left the foreground, such as for another app: a time to stop timers and animations.</summary>
    protected virtual void SlintPause()
    {
    }

    /// <summary>The activity came back to the foreground, and when it first started.</summary>
    protected virtual void SlintResume()
    {
    }

    /// <summary>The system is short of memory: a time to free caches.</summary>
    protected virtual void SlintLowMemory()
    {
    }

    /// <inheritdoc/>
    protected override void OnCreate(Bundle? savedInstanceState)
    {
        s_current = this;
        NativeMethods.slint_dotnet_android_set_callbacks(new AndroidCallbacks
        {
            Main = &Main,
            Event = &Event,
            SaveState = &SaveState,
            RestoreState = &RestoreState,
        });
        base.OnCreate(savedInstanceState);
    }

    /// <inheritdoc/>
    protected override void OnDestroy()
    {
        base.OnDestroy();
        // A second android_main in this process would run on a new thread, which can't use Slint.
        global::Android.OS.Process.KillProcess(global::Android.OS.Process.MyPid());
    }

    [UnmanagedCallersOnly]
    private static void Main()
    {
        try
        {
            var activity = s_current!;
            GeneratedApi.BaseDirectory = ExtractAssets(activity.Assets!, Path.Combine(activity.FilesDir!.AbsolutePath, AssetDirectory));
            activity.SlintMain();
        }
        catch (Exception e)
        {
            // Returning from android_main finishes the activity.
            Log.Error(LogTag, e.ToString());
        }
    }

    [UnmanagedCallersOnly]
    private static void Event(int kind)
    {
        try
        {
            // The values of ActivityEvent in android.rs.
            switch (kind)
            {
                case 0:
                    s_current!.SlintPause();
                    break;
                case 1:
                    s_current!.SlintResume();
                    break;
                case 2:
                    s_current!.SlintLowMemory();
                    break;
            }
        }
        catch (Exception e)
        {
            Log.Error(LogTag, e.ToString());
        }
    }

    [UnmanagedCallersOnly]
    private static void SaveState(nint state)
    {
        try
        {
            var bytes = s_current!.SlintSaveState();
            if (bytes is { Length: > 0 })
            {
                fixed (byte* data = bytes)
                {
                    NativeMethods.slint_dotnet_android_state_set(state, data, (nuint)bytes.Length);
                }
            }
        }
        catch (Exception e)
        {
            Log.Error(LogTag, e.ToString());
        }
    }

    [UnmanagedCallersOnly]
    private static void RestoreState(byte* data, nuint len)
    {
        try
        {
            s_current!.SlintRestoreState(new ReadOnlySpan<byte>(data, checked((int)len)).ToArray());
        }
        catch (Exception e)
        {
            Log.Error(LogTag, e.ToString());
        }
    }

    /// <summary>Replaces <paramref name="destination"/> with the assets under <see cref="AssetDirectory"/>.</summary>
    private static string ExtractAssets(AssetManager assets, string destination)
    {
        if (Directory.Exists(destination))
        {
            Directory.Delete(destination, recursive: true);
        }
        Directory.CreateDirectory(destination);
        foreach (var name in assets.List(AssetDirectory) ?? [])
        {
            Extract(assets, $"{AssetDirectory}/{name}", Path.Combine(destination, name));
        }
        return destination;

        // AssetManager.List gives no children for a file.
        static void Extract(AssetManager assets, string path, string destination)
        {
            var children = assets.List(path) ?? [];
            if (children.Length == 0)
            {
                using var input = assets.Open(path);
                using var output = File.Create(destination);
                input.CopyTo(output);
                return;
            }
            Directory.CreateDirectory(destination);
            foreach (var child in children)
            {
                Extract(assets, $"{path}/{child}", Path.Combine(destination, child));
            }
        }
    }
}
