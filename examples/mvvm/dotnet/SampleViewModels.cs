// Copyright © .NET Foundation and Contributors
// SPDX-License-Identifier: MIT

// The view models of https://github.com/CommunityToolkit/MVVM-Samples, adapted to show their state as text
// and to take no services.

using System.ComponentModel.DataAnnotations;
using System.Windows.Input;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using CommunityToolkit.Mvvm.Messaging;
using CommunityToolkit.Mvvm.Messaging.Messages;
using Slint;

namespace MvvmSample;

[SlintViewModel(typeof(ObservableObjectPage))]
public sealed class ObservableObjectPageViewModel : ObservableObject
{
    public ObservableObjectPageViewModel()
    {
        ReloadTaskCommand = new RelayCommand(ReloadTask);
    }

    public ICommand ReloadTaskCommand { get; }

    private string name = "";

    public string Name
    {
        get => name;
        set => SetProperty(ref name, value);
    }

    private TaskNotifier? myTask;

    public Task? MyTask
    {
        get => myTask;
        private set => SetPropertyAndNotifyOnCompletion(ref myTask, value);
    }

    public string TaskStatus => MyTask?.Status.ToString() ?? "None";

    public void ReloadTask()
    {
        MyTask = Task.Delay(3000);
    }
}

[SlintViewModel(typeof(RelayCommandPage))]
public sealed class RelayCommandPageViewModel : ObservableObject
{
    public RelayCommandPageViewModel()
    {
        IncrementCounterCommand = new RelayCommand(IncrementCounter);
    }

    public ICommand IncrementCounterCommand { get; }

    private int counter;

    public int Counter
    {
        get => counter;
        private set => SetProperty(ref counter, value);
    }

    private void IncrementCounter() => Counter++;
}

[SlintViewModel(typeof(AsyncRelayCommandPage))]
public sealed partial class AsyncRelayCommandPageViewModel : ObservableObject
{
    [ObservableProperty]
    private string _text = "";

    [RelayCommand]
    private async Task DownloadText()
    {
        await Task.Delay(3000); // Simulate a web request
        Text = "Hello world!";
    }
}

[SlintViewModel(typeof(MessengerPage))]
public sealed class MessengerPageViewModel : ObservableObject
{
    public MessengerPageViewModel()
    {
        RequestCurrentUsernameCommand = new RelayCommand(RequestCurrentUsername);
        ResetCurrentUsernameCommand = new RelayCommand(ResetCurrentUsername);
    }

    public ICommand RequestCurrentUsernameCommand { get; }
    public ICommand ResetCurrentUsernameCommand { get; }

    public UserSenderViewModel SenderViewModel { get; } = new();

    public UserReceiverViewModel ReceiverViewModel { get; } = new();

    private string username = "";

    public string Username
    {
        get => username;
        private set => SetProperty(ref username, value);
    }

    public void RequestCurrentUsername()
    {
        Username = WeakReferenceMessenger.Default.Send<CurrentUsernameRequestMessage>();
    }

    public void ResetCurrentUsername()
    {
        Username = "";
    }
}

[SlintViewModel(typeof(MessengerSender))]
public sealed class UserSenderViewModel : ObservableRecipient
{
    public UserSenderViewModel()
    {
        SendUserMessageCommand = new RelayCommand(SendUserMessage);
    }

    public ICommand SendUserMessageCommand { get; }

    private string username = "Bob";

    public string Username
    {
        get => username;
        private set => SetProperty(ref username, value);
    }

    protected override void OnActivated()
    {
        Messenger.Register<UserSenderViewModel, CurrentUsernameRequestMessage>(this, (r, m) => m.Reply(r.Username));
    }

    public void SendUserMessage()
    {
        Username = Username == "Bob" ? "Alice" : "Bob";

        Messenger.Send(new UsernameChangedMessage(Username));
    }
}

[SlintViewModel(typeof(MessengerReceiver))]
public sealed class UserReceiverViewModel : ObservableRecipient
{
    private string username = "";

    public string Username
    {
        get => username;
        private set => SetProperty(ref username, value);
    }

    protected override void OnActivated()
    {
        Messenger.Register<UserReceiverViewModel, UsernameChangedMessage>(this, (r, m) => r.Username = m.Value);
    }
}

public sealed class UsernameChangedMessage(string value) : ValueChangedMessage<string>(value);

public sealed class CurrentUsernameRequestMessage : RequestMessage<string>;

[SlintViewModel(typeof(ValidationPage))]
public sealed partial class ValidationFormViewModel : ObservableValidator
{
    [ObservableProperty]
    [NotifyDataErrorInfo]
    [Required]
    [MinLength(2)]
    [MaxLength(100)]
    private string _firstName = "";

    [ObservableProperty]
    [NotifyDataErrorInfo]
    [Required]
    [MinLength(2)]
    [MaxLength(100)]
    private string _lastName = "";

    [ObservableProperty]
    [NotifyDataErrorInfo]
    [Required]
    [EmailAddress]
    private string _email = "";

    [ObservableProperty]
    [NotifyDataErrorInfo]
    [Required]
    [Phone]
    private string _phoneNumber = "";

    [ObservableProperty]
    private string _status = "";

    [RelayCommand]
    private void Submit()
    {
        ValidateAllProperties();
        Status = HasErrors ? "Fix the errors above." : "Submitted.";
    }
}
