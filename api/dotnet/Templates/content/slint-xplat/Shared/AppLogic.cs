namespace SlintApp;

public static class AppLogic
{
    public static void Connect(AppWindow app)
    {
        app.OnRequestIncreaseValue(() => app.Counter++);
    }
}
