// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using Slint;

namespace TodoMvc.Mvc;

/// <summary>The logic of the task list page.</summary>
public sealed class TaskListController(ITaskRepository repository)
{
    private readonly TaskList _tasks = new(repository);

    /// <summary>Raised when the user asks to create a task.</summary>
    public event Action? ShowCreateTask;

    /// <summary>The tasks, as a model that follows the changes.</summary>
    public Model<TaskModel> Tasks => _tasks;

    public void ToggleDone(int index) => _tasks.ToggleDone(index);

    public void RemoveTask(int index) => _tasks.Remove(index);

    public void CreateTask(string title, DateTimeOffset dueDate) => _tasks.Add(new TaskModel(title, dueDate));

    public void RequestCreateTask() => ShowCreateTask?.Invoke();

    /// <summary>The tasks of the repository, which notifies the UI of each change it makes.</summary>
    private sealed class TaskList(ITaskRepository repository) : Model<TaskModel>
    {
        public override int RowCount => repository.Count;

        public override TaskModel RowData(int row) => repository.Get(row) ?? throw new ArgumentOutOfRangeException(nameof(row));

        public void ToggleDone(int index)
        {
            if (repository.ToggleDone(index))
            {
                NotifyRowChanged(index);
            }
        }

        public void Remove(int index)
        {
            if (repository.Remove(index))
            {
                NotifyRowRemoved(index, 1);
            }
        }

        public void Add(TaskModel task)
        {
            if (repository.Add(task))
            {
                NotifyRowAdded(RowCount - 1, 1);
            }
        }
    }
}

/// <summary>The logic of the page that creates a task.</summary>
public sealed class CreateTaskController(IDateTimeRepository repository)
{
    /// <summary>Raised when the user leaves the page.</summary>
    public event Action? Back;

    public DateOnly CurrentDate => repository.CurrentDate;

    public TimeOnly CurrentTime => repository.CurrentTime;

    public string Format(DateOnly date) => repository.Format(date);

    public string Format(TimeOnly time) => repository.Format(time);

    public DateTimeOffset ToDateTime(DateOnly date, TimeOnly time) => repository.ToDateTime(date, time);

    public void GoBack() => Back?.Invoke();
}
