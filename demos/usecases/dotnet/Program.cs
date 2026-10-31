// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Globalization;
using Slint;
using Usecases;

internal static class Program
{
    private const string Message =
        "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. " +
        "Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo consequat.";

    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var app = App.Create();

        // The strings of the .slint files are English; German is bundled.
        if (CultureInfo.CurrentUICulture.TwoLetterISOLanguageName == "de")
        {
            Translations.SelectBundledTranslation("de");
        }

        // A key of the on-screen keyboard acts like the same key on a real one.
        app.VirtualKeyboardHandler.Enabled = true;
        app.VirtualKeyboardHandler.OnKeyPressed(key =>
        {
            app.Window.DispatchEvent(new WindowEvent.KeyPressed(key));
            app.Window.DispatchEvent(new WindowEvent.KeyReleased(key));
        });

        var mails = new ListModel<CardListViewItem>(new (string Sender, string Received)[]
        {
            ("Simon Hausmann", "1 hour ago"),
            ("Tobias Hunger", "1 day ago"),
            ("Olivier Goffart", "1 day"),
            ("Aurindam Jana", "2 hour ago"),
            ("Simon Hausmann", "5 hour ago"),
            ("Tobias Hunger", "7 hours ago"),
            ("Olivier Goffart", "8 hour ago"),
            ("Aurindam Jana", "9 hour ago"),
        }.Select(mail => new CardListViewItem { Title = mail.Sender, Note = mail.Received, SubTitle = "Meeting tomorrow", Caption = Message }));

        var mailBox = app.MailBoxViewAdapter;
        mailBox.Mails = mails;
        mailBox.OnSearchTextChanged(text =>
            mailBox.Mails = mails.Filter(mail => mail.Title.Contains(text, StringComparison.CurrentCultureIgnoreCase)));

        app.Run();
    }
}
