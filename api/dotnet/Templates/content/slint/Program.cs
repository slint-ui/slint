namespace SlintApp;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var app = AppWindow.Create();
#if (mvvm)
        // Binds the members of the view model to the properties and callbacks of the same name.
        using var binding = app.Bind(new MainViewModel());
#else
        app.OnRequestIncreaseValue(() => app.Counter++);
#endif
        app.Run();
    }
}
