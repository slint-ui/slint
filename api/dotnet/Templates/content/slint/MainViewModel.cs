using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Slint;

namespace SlintApp;

[SlintViewModel(typeof(AppWindow))]
public sealed partial class MainViewModel : ObservableObject
{
    // The property `counter` of the window.
    [ObservableProperty]
    private int _counter = 42;

    // The callback `request-increase-value` of the window.
    [RelayCommand]
    private void RequestIncreaseValue() => Counter++;
}
