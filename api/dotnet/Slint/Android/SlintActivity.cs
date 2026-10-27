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

    /// <inheritdoc/>
    protected override void OnCreate(Bundle? savedInstanceState)
    {
        s_current = this;
        NativeMethods.slint_dotnet_android_set_main(&Main);
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
