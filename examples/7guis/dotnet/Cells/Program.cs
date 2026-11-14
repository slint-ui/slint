// Copyright © SixtyFPS GmbH <info@slint.dev>
// SPDX-License-Identifier: MIT

using System.Globalization;
using Cells;
using Slint;

internal static class Program
{
    // Windows needs a single-threaded apartment for the window.
    [STAThread]
    private static void Main()
    {
        using var window = MainWindow.Create();
        var sheet = new Sheet(rows: 100, columns: 26);
        window.Cells = new ListModel<Model<CellContent>>(sheet.Rows);
        window.Run();
    }
}

/// <summary>A formula: a number, a cell such as <c>B4</c>, or a sum or difference of formulas.</summary>
internal abstract record Expr
{
    public sealed record Value(float Number) : Expr;

    public sealed record Cell(int Row, int Column) : Expr;

    public sealed record Add(Expr Left, Expr Right) : Expr;

    public sealed record Sub(Expr Left, Expr Right) : Expr;

    /// <summary>Parses formulas such as <c>42</c>, <c>B4</c>, and <c>D22 + B12 - 85.5</c>, or returns null.</summary>
    public static Expr? Parse(string formula)
    {
        formula = formula.Trim();
        var letters = formula.TakeWhile(char.IsAsciiLetter).Count();
        var digits = formula.Skip(letters).TakeWhile(c => char.IsAsciiDigit(c) || c == '.').Count();
        Expr operand;
        if (letters == 1 && int.TryParse(formula.AsSpan(1, digits), CultureInfo.InvariantCulture, out var row))
        {
            operand = new Cell(row, char.ToLowerInvariant(formula[0]) - 'a');
        }
        else if (letters == 0 && float.TryParse(formula.AsSpan(0, digits), CultureInfo.InvariantCulture, out var number))
        {
            operand = new Value(number);
        }
        else
        {
            return null;
        }
        var rest = formula[(letters + digits)..].Trim();
        if (rest.Length == 0)
        {
            return operand;
        }
        var right = Parse(rest[1..]);
        return right is null ? null : rest[0] switch
        {
            '+' => new Add(operand, right),
            '-' => new Sub(operand, right),
            _ => null,
        };
    }

    public IEnumerable<Cell> Cells() => this switch
    {
        Cell cell => [cell],
        Add(var left, var right) => left.Cells().Concat(right.Cells()),
        Sub(var left, var right) => left.Cells().Concat(right.Cells()),
        _ => [],
    };
}

/// <summary>The cells of the spreadsheet, which compute their values from their formulas.</summary>
internal sealed class Sheet
{
    // The cells whose formulas use each cell, to compute them again when it changes.
    private readonly Dictionary<(int Row, int Column), HashSet<(int Row, int Column)>> _dependents = [];
    private readonly Dictionary<(int Row, int Column), Expr> _formulas = [];

    public Sheet(int rows, int columns) =>
        Rows = Enumerable.Range(0, rows).Select(row => new SheetRow(this, row, columns)).ToArray();

    public SheetRow[] Rows { get; }

    /// <summary>Sets the formula of a cell, or with null computes its current formula again.</summary>
    public void Update(int row, int column, string? formula)
    {
        var cells = Rows[row].Cells;
        var cell = cells[column];
        if (formula is not null)
        {
            cell = cell with { Formula = formula };
        }
        if (_formulas.Remove((row, column), out var old))
        {
            foreach (var used in old.Cells())
            {
                _dependents.GetValueOrDefault((used.Row, used.Column))?.Remove((row, column));
            }
        }
        var expr = Expr.Parse(cell.Formula) ?? new Expr.Value(0);
        var value = Evaluate(expr);
        var changed = value != cell.Value;
        cells[column] = cell with { Value = value };
        if (changed || formula is not null)
        {
            Rows[row].Changed(column);
        }
        if (changed)
        {
            foreach (var (dependentRow, dependentColumn) in _dependents.GetValueOrDefault((row, column))?.ToList() ?? [])
            {
                Update(dependentRow, dependentColumn, null);
            }
        }
        _formulas[(row, column)] = expr;
        foreach (var used in expr.Cells())
        {
            if (!_dependents.TryGetValue((used.Row, used.Column), out var dependents))
            {
                _dependents[(used.Row, used.Column)] = dependents = [];
            }
            dependents.Add((row, column));
        }
    }

    private float Evaluate(Expr expr) => expr switch
    {
        Expr.Value(var number) => number,
        Expr.Cell(var row, var column) => row < Rows.Length && column >= 0 && column < Rows[row].Cells.Length ? Rows[row].Cells[column].Value : 0,
        Expr.Add(var left, var right) => Evaluate(left) + Evaluate(right),
        Expr.Sub(var left, var right) => Evaluate(left) - Evaluate(right),
        _ => 0,
    };
}

/// <summary>A row of the spreadsheet; editing a cell sets its formula.</summary>
internal sealed class SheetRow(Sheet sheet, int row, int columns) : Model<CellContent>
{
    public CellContent[] Cells { get; } = Enumerable.Repeat(new CellContent(), columns).ToArray();

    public override int RowCount => Cells.Length;

    public override CellContent RowData(int column) => Cells[column];

    public override void SetRowData(int column, CellContent value) => sheet.Update(row, column, value.Formula);

    public void Changed(int column) => NotifyRowChanged(column);
}
