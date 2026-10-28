namespace SlintApp;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var app = AppWindow.Create();
        app.OnRequestIncreaseValue(() => app.Counter++);
        app.Run();
    }
}
