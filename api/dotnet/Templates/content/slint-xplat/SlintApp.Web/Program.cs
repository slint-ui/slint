namespace SlintApp;

internal static class Program
{
    private static async Task Main()
    {
        using var app = AppWindow.Create();
        AppLogic.Connect(app);
        // The browser's thread can't block, so the app runs while RunAsync awaits.
        await app.RunAsync();
    }
}
