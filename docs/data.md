# Data Storage

All data is stored in `.bmo/issues.db` in the directory where `bmo init` was run.

`bmo` walks up the directory tree from the current working directory to find the nearest `.bmo/` directory, so you can run `bmo` commands from any subdirectory of your project.

## Overriding the Database Path

Set the `BMO_DB` environment variable or pass `--db <path>` to use a database at an explicit path:

```bash
BMO_DB=/path/to/issues.db bmo issue list
bmo --db /path/to/issues.db issue list
```

When both are set, `--db` takes precedence.
