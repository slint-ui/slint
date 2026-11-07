// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

namespace TodoMvc.Mvc;

/// <summary>Where the tasks are stored.</summary>
public interface ITaskRepository
{
    int Count { get; }

    TaskModel? Get(int index);

    bool ToggleDone(int index);

    bool Remove(int index);

    bool Add(TaskModel task);
}

/// <summary>The clock and the formats of dates and times.</summary>
public interface IDateTimeRepository
{
    DateOnly CurrentDate { get; }

    TimeOnly CurrentTime { get; }

    string Format(DateOnly date);

    string Format(TimeOnly time);

    DateTimeOffset ToDateTime(DateOnly date, TimeOnly time);
}

/// <summary>Tasks in memory, starting with a few examples.</summary>
public sealed class MockTaskRepository(IEnumerable<TaskModel> tasks) : ITaskRepository
{
    private readonly List<TaskModel> _tasks = [.. tasks];

    public int Count => _tasks.Count;

    public TaskModel? Get(int index) => index >= 0 && index < _tasks.Count ? _tasks[index] : null;

    public bool ToggleDone(int index)
    {
        if (Get(index) is not { } task)
        {
            return false;
        }
        _tasks[index] = task with { Done = !task.Done };
        return true;
    }

    public bool Remove(int index)
    {
        if (Get(index) is null)
        {
            return false;
        }
        _tasks.RemoveAt(index);
        return true;
    }

    public bool Add(TaskModel task)
    {
        _tasks.Add(task);
        return true;
    }
}

/// <summary>A fixed clock, so that the example looks the same every time.</summary>
public sealed class MockDateTimeRepository(DateOnly date, TimeOnly time) : IDateTimeRepository
{
    public DateOnly CurrentDate => date;

    public TimeOnly CurrentTime => time;

    public string Format(DateOnly value) => $"{value.Year}/{value.Month}/{value.Day}";

    public string Format(TimeOnly value) => $"{value.Hour}:{value.Minute}";

    public DateTimeOffset ToDateTime(DateOnly dateValue, TimeOnly timeValue) => new(dateValue.ToDateTime(timeValue), TimeSpan.Zero);
}

public static class Repositories
{
    public static ITaskRepository Tasks() => new MockTaskRepository(
    [
        new("Learn C#", DateTimeOffset.FromUnixTimeMilliseconds(1717686537151), Done: true),
        new("Learn Slint", DateTimeOffset.FromUnixTimeMilliseconds(1717686537151), Done: true),
        new("Create project with C# and Slint", DateTimeOffset.FromUnixTimeMilliseconds(1717686537151), Done: true),
    ]);

    public static IDateTimeRepository DateTime() => new MockDateTimeRepository(new DateOnly(2024, 6, 11), new TimeOnly(16, 43));
}
