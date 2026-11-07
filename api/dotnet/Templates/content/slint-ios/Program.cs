namespace SlintApp;

internal static class Program
{
    private static void Main()
    {
        using var app = AppWindow.Create();
        app.OnRequestIncreaseValue(() => app.Counter++);
        // Slint starts the app here, as Info.plist's scene delegate says, and Run doesn't return.
        app.Run();
    }
}
