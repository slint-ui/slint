// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using MvvmSample;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        var messenger = new MessengerPageViewModel();
        messenger.SenderViewModel.IsActive = true;
        messenger.ReceiverViewModel.IsActive = true;

        using var bindings = new CompositeBindings(
            window.ObservableObjectPage.Bind(new ObservableObjectPageViewModel()),
            window.RelayCommandPage.Bind(new RelayCommandPageViewModel()),
            window.AsyncRelayCommandPage.Bind(new AsyncRelayCommandPageViewModel()),
            window.MessengerPage.Bind(messenger),
            window.MessengerSender.Bind(messenger.SenderViewModel),
            window.MessengerReceiver.Bind(messenger.ReceiverViewModel),
            window.CollectionsPage.Bind(new CollectionsPageViewModel()),
            window.ValidationPage.Bind(new ValidationFormViewModel()));
        window.Run();
    }

    private sealed class CompositeBindings(params IDisposable[] bindings) : IDisposable
    {
        public void Dispose()
        {
            foreach (var binding in bindings)
            {
                binding.Dispose();
            }
        }
    }
}
