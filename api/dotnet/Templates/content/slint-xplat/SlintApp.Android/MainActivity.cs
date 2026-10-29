using Slint.Android;

namespace SlintApp;

[Activity(Label = "SlintApp", MainLauncher = true, Exported = true, ConfigurationChanges = SlintActivity.ConfigurationChanges)]
[MetaData(SlintActivity.LibraryNameKey, Value = SlintActivity.LibraryName)]
public class MainActivity : SlintActivity
{
    // Runs on the Slint thread, like every callback of the components.
    protected override void SlintMain()
    {
        using var app = AppWindow.Create();
        AppLogic.Connect(app);
        app.Run();
    }
}
