// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;
using Slint;

namespace MvvmSample;

[SlintViewModel(typeof(CollectionsPage))]
public sealed partial class CollectionsPageViewModel : ObservableObject
{
    public ObservableCollection<string> Contacts { get; } = ["Ada Lovelace", "Alan Turing", "Grace Hopper"];

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(AddContactCommand))]
    private string _newContact = "";

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(RemoveContactCommand))]
    private int _selectedContact = -1;

    [RelayCommand(CanExecute = nameof(CanAddContact))]
    private void AddContact()
    {
        Contacts.Add(NewContact.Trim());
        NewContact = "";
    }

    private bool CanAddContact() => !string.IsNullOrWhiteSpace(NewContact);

    [RelayCommand(CanExecute = nameof(CanRemoveContact))]
    private void RemoveContact()
    {
        Contacts.RemoveAt(SelectedContact);
        SelectedContact = -1;
    }

    private bool CanRemoveContact() => SelectedContact >= 0 && SelectedContact < Contacts.Count;
}
