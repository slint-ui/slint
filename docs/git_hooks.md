# Setting up Git Hooks with Mise

This project supports using `mise` to run autofix tasks automatically before committing changes. This ensures that all code complies with formatting and linting rules.

> **Prerequisite:** Ensure you have `mise` installed and set up. See [Environment Setup in `docs/development.md`](development.md#environment-setup) for instructions.

## Method: Native Git Hook

We use a simple shell script in `.git/hooks/pre-commit` to invoke `mise`.

### 1. Create the hook script

Create or edit `.git/hooks/pre-commit` with the following content:

```bash
#!/bin/sh
# pre-commit hook to run slint autofix

# Check if mise is installed
if ! command -v mise >/dev/null 2>&1; then
    echo "mise not found. Skipping autofix."
    exit 0
fi

echo "Running slint autofix (mise run ci:autofix:fix)..."
if ! mise run ci:autofix:fix; then
    echo "Autofix command failed."
    exit 1
fi

# Check for unstaged changes
# If the autofix modified any files, they will appear as unstaged changes.
# We abort the commit if there are any diffs to ensure the user acknowledges the formatting changes.
if ! git diff --quiet; then
    echo "--------------------------------------------------------"
    echo "Autofix detected/made changes to files."
    echo "Please inspect the changes, 'git add' the corrected files, and commit again."
    echo "--------------------------------------------------------"
    exit 1
fi
```

### 2. Make it executable

Run the following command to make the script executable:

```bash
chmod +x .git/hooks/pre-commit
```

### Windows Users

For Windows, the process is similar but requires attention to file saving and environment.

1. **Use Git Bash**: The hook is a shell script (`#!/bin/sh`), so it runs natively in the Git Bash environment that comes with Git for Windows.
2. **File Location**: Save the file as `.git/hooks/pre-commit`. **Make sure it has NO file extension** (e.g., not `pre-commit.txt`).
3. **Mise Path**: Ensure `mise` is in your system PATH or accessible from Git Bash.
4. **Encoding**: Ensure the file uses **LF (Line Feed)** line endings, not CRLF. Most modern editors (VS Code, Notepad++) allow you to set this.

If you are using PowerShell to create the file, you can use:

```powershell
Set-Content -Path .git/hooks/pre-commit -Value '#!/bin/sh
# ... (paste the script content here) ...
' -NoNewline
```

*Note: The script itself does not need `chmod +x` on Windows.*
```

## How it works

1. When you run `git commit`, the hook triggers `mise run ci:autofix:fix`.
2. This runs all formatters (Rust, Python, JS, etc.) defined in the CI configuration.
3. If the formatters makes changes:
   - The commit is **aborted**.
   - You will see a message asking you to review the changes.
   - Use `git add -u` (or add specific files) to stage the fixes.
   - Run `git commit` again.

## Disabling the hook

To bypass the hook for a specific commit (not recommended), use the `--no-verify` flag:

```bash
git commit --no-verify -m "Commit message"
```
