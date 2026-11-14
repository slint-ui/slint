// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using TodoMvc;
using TodoMvc.Mvc;
using TodoMvc.Ui;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        var taskList = new TaskListController(Repositories.Tasks());
        var createTask = new CreateTaskController(Repositories.DateTime());
        Adapters.Connect(window, taskList, createTask);
        window.Run();
    }
}
