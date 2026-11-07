// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

namespace TodoMvc.Mvc;

/// <summary>A task of the todo list.</summary>
public sealed record TaskModel(string Title, DateTimeOffset DueDate, bool Done = false);
