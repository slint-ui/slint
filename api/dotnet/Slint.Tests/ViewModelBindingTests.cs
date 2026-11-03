// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0

using System.Collections.ObjectModel;
using CommunityToolkit.Mvvm.ComponentModel;
using CommunityToolkit.Mvvm.Input;

namespace Slint.Tests;

[SlintViewModel(typeof(PersonForm))]
public sealed partial class PersonViewModel : ObservableObject
{
    [ObservableProperty]
    private string _name = "Ada";

    [ObservableProperty]
    [NotifyCanExecuteChangedFor(nameof(SaveCommand))]
    private int _age;

    public ObservableCollection<string> Tags { get; } = ["a"];

    public int Saved { get; private set; }

    /// <summary>The out property tag-count, from the UI.</summary>
    public int TagCount { get; set; }

    [RelayCommand(CanExecute = nameof(CanSave))]
    private void Save() => Saved++;

    private bool CanSave() => Age >= 18;
}

[TestClass]
public sealed class ViewModelBindingTests
{
    private const string Source = """
        export component App {
            in-out property <string> name;
            in property <int> age;
            in property <[string]> tags;
            in property <bool> save-enabled;
            out property <int> tag-count: tags.length;
            callback save();
            public function type-name(text: string) { name = text; }
            public function click-save() { save(); }
        }
        """;

    [TestMethod]
    public void BindsPropertiesCollectionsAndCommands() => SlintThread.Run(() =>
    {
        using var app = ModelTests.Create(Source);
        var vm = new PersonViewModel();
        using (app.Bind(vm)
            .Property("name", v => v.Name, (v, value) => v.Name = value)
            .Property("age", v => v.Age)
            .Collection("tags", v => v.Tags)
            .Command("save", v => v.SaveCommand, enabledProperty: "save-enabled"))
        {
            Assert.AreEqual("Ada", app.GetProperty("name"));
            Assert.IsFalse((bool)app.GetProperty("save-enabled")!);

            vm.Name = "Bob";
            Assert.AreEqual("Bob", app.GetProperty("name"));

            // The user's input reaches the view model once Slint runs its change handlers.
            app.Invoke("type-name", "Cy");
            Testing.MockElapsedTime(0);
            Assert.AreEqual("Cy", vm.Name);

            app.Invoke("click-save");
            Assert.AreEqual(0, vm.Saved);
            vm.Age = 20;
            Assert.IsTrue((bool)app.GetProperty("save-enabled")!);
            app.Invoke("click-save");
            Assert.AreEqual(1, vm.Saved);

            vm.Tags.Add("b");
            Assert.AreEqual(2, Convert.ToInt32(app.GetProperty("tag-count")));

            EventLoop.Run(async () =>
            {
                await Task.Run(() => vm.Name = "Dee");
                Assert.AreEqual("Dee", app.GetProperty("name"));
            });
        }

        vm.Name = "Eve";
        Assert.AreEqual("Dee", app.GetProperty("name"));
    });

    [TestMethod]
    public void GeneratedBindMatchesMembersByName() => SlintThread.Run(() =>
    {
        using var form = PersonForm.Create();
        var vm = new PersonViewModel();
        using (form.Bind(vm))
        {
            Assert.AreEqual("Ada", form.Name);
            Assert.IsFalse(form.SaveEnabled);
            Assert.AreEqual(1, vm.TagCount);

            // A float property of the component, from an int of the view model.
            vm.Age = 20;
            Assert.AreEqual(20f, form.Age);
            Assert.IsTrue(form.SaveEnabled);

            form.TypeName("Cy");
            Testing.MockElapsedTime(0);
            Assert.AreEqual("Cy", vm.Name);

            form.ClickSave();
            Assert.AreEqual(1, vm.Saved);

            vm.Tags.Add("b");
            Testing.MockElapsedTime(0);
            Assert.AreEqual(2, vm.TagCount);
        }
    });
}
