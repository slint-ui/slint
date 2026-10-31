// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Globalization;
using Slint;
using TodoMvc.Mvc;

namespace TodoMvc.Ui;

/// <summary>Connects the controllers to the adapter globals of the UI, and maps the types between them.</summary>
public static class Adapters
{
    public static void Connect(MainWindow window, TaskListController taskList, CreateTaskController createTask)
    {
        var tasks = window.TaskListAdapter;
        tasks.Tasks = taskList.Tasks.Map(task => new SelectionListViewItem
        {
            Text = task.Title,
            Checked = task.Done,
            Description = task.DueDate.ToString("ddd, MMM dd, yyyy HH:mm", CultureInfo.InvariantCulture),
        });
        tasks.OnToggleTaskChecked(taskList.ToggleDone);
        tasks.OnRemoveTask(taskList.RemoveTask);
        tasks.OnShowCreateTask(taskList.RequestCreateTask);

        var create = window.CreateTaskAdapter;
        create.OnBack(createTask.GoBack);
        create.OnCurrentDate(() => ToUi(createTask.CurrentDate));
        create.OnCurrentTime(() => ToUi(createTask.CurrentTime));
        create.OnDateString(date => createTask.Format(ToDate(date)));
        create.OnTimeString(time => createTask.Format(ToTime(time)));
        // A Slint int holds seconds since 1970, not milliseconds.
        create.OnTimeStamp((date, time) => (int)createTask.ToDateTime(ToDate(date), ToTime(time)).ToUnixTimeSeconds());
        create.OnCreate((title, seconds) => taskList.CreateTask(title, DateTimeOffset.FromUnixTimeSeconds(seconds)));

        taskList.ShowCreateTask += window.NavigationAdapter.NextPage;
        createTask.Back += window.NavigationAdapter.PreviousPage;
    }

    private static Date ToUi(DateOnly date) => new() { Year = date.Year, Month = date.Month, Day = date.Day };

    private static Time ToUi(TimeOnly time) => new() { Hour = time.Hour, Minute = time.Minute, Second = time.Second };

    private static DateOnly ToDate(Date date) => new(date.Year, date.Month, date.Day);

    private static TimeOnly ToTime(Time time) => new(time.Hour, time.Minute, time.Second);
}
