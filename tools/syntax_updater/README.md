# Slint syntax updater

Use this tool to update Slint files from the current Syntax to the new experimental syntax, using the new
component declaration and following the new rules tracked in [issue #1750](https://github.com/slint-ui/slint/issues/1750)

### Usage:

```
export SLINT_EXPERIMENTAL_SYNTAX=true
cargo run -p syntax_updater -- -i /path/to/my/app/ui/**/*.slint
```

