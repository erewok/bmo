# bmo Command Reference

This file is a machine-readable reference for every bmo command. It is intended for AI agents,
scripts, and tools that need to understand the full CLI surface without running the binary.

All commands accept the global flags `--json` and `--db`. All enumerated values are
case-insensitive. Issue IDs may be supplied as bare integers (`1`) or in `BMO-N` format
(`BMO-1`).

## JSON Output Envelope

All commands that produce structured output use a consistent envelope when `--json` is passed.

Success:

```json
{"ok": true, "data": <payload>, "message": "<human summary>"}
```

Error:

```json
{"ok": false, "error": "<description>", "code": "<code>"}
```

Error codes:

| Code | Exit status | Meaning |
|------|-------------|---------|
| `general` | 1 | Unclassified error |
| `not-found` | 2 | Requested resource does not exist |
| `validation` | 3 | Invalid input |
| `conflict` | 4 | Operation would violate a constraint |

`--json` output is designed for programmatic consumption. The envelope shape and field names are
stable across patch releases within a major version.

## Enumerated Values

Valid values for status, priority, kind, and relation fields. All values are case-insensitive.
`in-progress` also accepts the aliases `in_progress` and `inprogress`.

| Field | Valid values |
|-------|-------------|
| status | `backlog`, `todo`, `in-progress`, `review`, `done` |
| priority | `none`, `low`, `medium`, `high`, `critical` |
| kind | `bug`, `feature`, `task`, `epic`, `chore` |
| relation | `blocks`, `blocked-by`, `depends-on`, `dependency-of`, `relates-to`, `duplicates`, `duplicate-of` |

## Global Flags

These flags are accepted by every command.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--json` | bool | false | Output all results as JSON using the standard envelope |
| `--db <PATH>` | string | (auto-detected) | Override the database path. Also reads `BMO_DB` environment variable |

## bmo init

Initialize a new bmo project in the current directory.

**Synopsis:** `bmo init [--name <name>]`

Creates a `.bmo/` directory containing `issues.db` and `config.toml`. If the directory already
exists, the command is a no-op.

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--name <name>` | string | (none) | Project name to store in config.toml |

**Example:**

```
bmo init --name my-project
```

**JSON output** (`data` field):

```json
{
  "db_path": "/path/to/.bmo/issues.db",
  "already_existed": false
}
```

## bmo config

Show or modify project configuration. When called without flags, prints all current config values.

**Synopsis:** `bmo config [--get <key>] [--set <key=value>]`

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--get <key>` | string | (none) | Print the value of a single config key |
| `--set <key=value>` | string | (none) | Set a config key to a value (format: `key=value`) |

Supported config keys: `project_name`, `default_assignee`, `web_port`, `web_host`.

**Examples:**

```
bmo config
bmo config --get web_port
bmo config --set default_assignee=alice
```

**JSON output** (`data` field when listing all):

```json
{
  "project_name": "my-project",
  "default_assignee": null,
  "web_port": 7777,
  "web_host": "127.0.0.1"
}
```

## bmo version

Print the bmo version string.

**Synopsis:** `bmo version`

No flags beyond globals.

**Example:**

```
bmo version
```

**JSON output** (`data` field):

```json
{"version": "0.1.0"}
```

## bmo stats

Show issue statistics broken down by status, priority, and kind.

**Synopsis:** `bmo stats`

No flags beyond globals.

**Example:**

```
bmo stats
bmo stats --json
```

## bmo board

Show all issues organized into a Kanban board by status column.

**Synopsis:** `bmo board [--label <label>]... [--priority <priority>]... [--assignee <name>]`

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-l, --label <label>` | string (repeatable) | (none) | Filter board to issues with this label |
| `-p, --priority <priority>` | string (repeatable) | (none) | Filter board to this priority |
| `-a, --assignee <name>` | string | (none) | Filter board to this assignee |

**Examples:**

```
bmo board
bmo board --assignee alice --json
bmo board --priority high --priority critical
```

**JSON output** (`data` field):

```json
{
  "backlog": [ <issue>, ... ],
  "todo": [ <issue>, ... ],
  "in_progress": [ <issue>, ... ],
  "review": [ <issue>, ... ],
  "done": [ <issue>, ... ]
}
```

## bmo next

Show issues that are ready to work on — issues that have no unresolved blocking dependencies.

**Synopsis:** `bmo next [--assignee <name>] [--limit <n>]`

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-a, --assignee <name>` | string | (none) | Restrict results to this assignee |
| `--limit <n>` | integer | 10 | Maximum number of results |

**Examples:**

```
bmo next
bmo next --assignee alice --limit 5 --json
```

**JSON output** (`data` field): array of issue objects.

## bmo plan

Show a phased execution plan derived from issue dependency relationships. Issues are grouped into
phases where each phase contains issues that can be worked in parallel.

**Synopsis:** `bmo plan [--assignee <name>] [--phase <n>]`

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-a, --assignee <name>` | string | (none) | Filter plan to this assignee |
| `--phase <n>` | integer | (none) | Return only the issues in phase N as a flat array |

When `--phase` is given, the `data` field is a plain array of issue objects (not the full plan
envelope), suitable for a coordinator to iterate and dispatch. An out-of-range phase number
returns a `validation` error (exit 3).

When `--assignee` and `--phase` are both given, phase N is extracted from the full plan first,
then the result is filtered by assignee within that phase. An empty result is not an error.
`--assignee` requires `--phase`; using `--assignee` without `--phase` is a validation error (exit 3).

**Examples:**

```
bmo plan --json
bmo plan --phase 1 --json
bmo plan --phase 2 --assignee alice --json
```

**JSON output** (`data` field, without `--phase`):

```json
{
  "total_issues": 12,
  "total_phases": 3,
  "max_parallelism": 4,
  "phases": [
    {"number": 1, "issues": [ <issue>, ... ]},
    ...
  ]
}
```

**JSON output** (with `--phase N`):

```json
{
  "ok": true,
  "data": [
    {"id": 3, "title": "...", "status": "todo", ...},
    {"id": 5, "title": "...", "status": "todo", ...}
  ],
  "message": "Phase 1: 2 issue(s)."
}
```

Out-of-range phase error (exit 3):

```json
{"ok": false, "code": "validation", "error": "phase 5 does not exist (plan has 3 phases)"}
```

## bmo export

Export all issues, comments, labels, relations, activity, and file attachments to JSON.

**Synopsis:** `bmo export [--output <file>]`

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-o, --output <file>` | string | (stdout) | Write output to this file path instead of stdout |

`--json` has no effect on this command; the output is always a JSON export bundle.

**Example:**

```
bmo export --output backup.json
bmo export > backup.json
```

## bmo import

Import issues from a JSON export file.

**Synopsis:** `bmo import <file> [--from-docket]`

| Argument/Flag | Type | Default | Description |
|---------------|------|---------|-------------|
| `<file>` | positional | (required) | Path to the JSON export file |
| `--from-docket` | bool | false | Treat the file as a docket-format export; remaps `DKT-` IDs to `BMO-` |

**Examples:**

```
bmo import backup.json
bmo import docket-export.json --from-docket --json
```

**JSON output** (`data` field):

```json
{"issues": 10, "comments": 3}
```

The envelope also includes a top-level `"warnings"` array listing any records that were skipped
due to unresolvable IDs.

## bmo truncate

Delete issues in bulk. Defaults to deleting all `done` issues if no filter is specified.

**Synopsis:** `bmo truncate [--status <status>]... [--all] [--yes]`

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-s, --status <status>` | string (repeatable) | `done` | Delete issues with this status (repeatable for multiple) |
| `--all` | bool | false | Delete ALL issues regardless of status (mutually exclusive with `--status`) |
| `--yes` | bool | false | Skip the confirmation prompt |

Prompts for confirmation before deleting unless `--yes` is passed. Reports the count of deleted issues.

**Examples:**

```
bmo truncate
bmo truncate --yes
bmo truncate --status backlog --status todo
bmo truncate --all --yes
```

**JSON output**:

```json
{"ok": true, "data": {"deleted": 47}, "message": "Deleted 47 issue(s)."}
{"ok": true, "data": {"deleted": 0}, "message": "Nothing to delete."}
```

## bmo web

Start the local read-only web UI for browsing issues in a browser.

**Synopsis:** `bmo web [--port <port>] [--host <host>] [--no-open]`

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-p, --port <port>` | integer | 7777 | TCP port to listen on |
| `--host <host>` | string | `127.0.0.1` | Host address to bind to |
| `--no-open` | bool | false | Do not automatically open a browser window |

`--json` has no effect on this command.

**Examples:**

```
bmo web
bmo web --port 8080 --no-open
bmo web --host 0.0.0.0 --port 7777
```

## bmo issue

Manage issues. All issue operations are subcommands of `bmo issue`.

### bmo issue create

Create a new issue.

**Synopsis:** `bmo issue create --title <title> [options]`

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-t, --title <title>` | string | (required) | Issue title |
| `-d, --description <text>` | string | `""` | Issue description |
| `-s, --status <status>` | string | `backlog` | Initial status |
| `-p, --priority <priority>` | string | `medium` | Priority |
| `-T, --kind <kind>` | string | `task` | Issue kind/type |
| `-a, --assignee <name>` | string | (none) | Assignee |
| `--parent <id>` | string | (none) | Parent issue ID |
| `-l, --label <label>` | string (repeatable) | (none) | Label to apply (repeat for multiple) |
| `-f, --file <path>` | string (repeatable) | (none) | File path to attach (repeat for multiple) |

**Examples:**

```
bmo issue create --title "Fix login bug" --kind bug --priority high
bmo issue create -t "Add tests" -T task --parent BMO-1 --label testing
bmo issue create --title "Epic: v2 launch" --kind epic --json
```

**JSON output** (`data` field): a single issue object.

### bmo issue list

List issues with optional filters. Excludes done issues by default.

**Synopsis:** `bmo issue list [options]`

Alias: `bmo issue ls`

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `-s, --status <status>` | string (repeatable) | (none) | Filter by status |
| `-p, --priority <priority>` | string (repeatable) | (none) | Filter by priority |
| `-T, --kind <kind>` | string (repeatable) | (none) | Filter by kind |
| `-a, --assignee <name>` | string | (none) | Filter by assignee |
| `-l, --label <label>` | string (repeatable) | (none) | Filter by label (AND semantics) |
| `--parent <id>` | string | (none) | Filter to children of this issue |
| `--search <text>` | string | (none) | Full-text search in title and description |
| `--limit <n>` | integer | 50 | Maximum number of results |
| `--sort <field>` | string | (none) | Sort field |
| `--include-done` | bool | false | Removes the default `status != done` exclusion; done issues are returned alongside active ones. All other filters (priority, kind, labels, etc.) remain active. Distinct from `--all` which short-circuits every predicate. |
| `--all` | bool | false | Return all issues regardless of status or other filters (short-circuits all predicates) |
| `--oneline` | bool | false | Print one compact line per issue (ID, status, priority, kind, title) |

**Examples:**

```
bmo issue list
bmo issue list --status in-progress --assignee alice
bmo issue list --kind bug --priority high --all --json
bmo issue list --search "login" --limit 10
bmo issue list --include-done --priority high
```

**JSON output** (`data` field): array of issue objects.

### bmo issue show

Show full details for a single issue, including sub-issues, relations, comments, and labels.

**Synopsis:** `bmo issue show <id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID (e.g. `1` or `BMO-1`) |

**Examples:**

```
bmo issue show BMO-5
bmo issue show 5 --json
```

**JSON output** (`data` field):

```json
{
  "issue": <issue object>,
  "sub_issues": [ <issue>, ... ],
  "relations": [ <relation>, ... ],
  "comments": [ <comment>, ... ],
  "labels": [ <label>, ... ]
}
```

### bmo issue edit

Edit one or more fields on an existing issue. Only the fields you supply are updated.

**Synopsis:** `bmo issue edit <id> [options]`

| Argument/Flag | Type | Default | Description |
|---------------|------|---------|-------------|
| `<id>` | positional (required) | | Issue ID |
| `-t, --title <title>` | string | (unchanged) | New title |
| `-d, --description <text>` | string | (unchanged) | New description |
| `-s, --status <status>` | string | (unchanged) | New status |
| `-p, --priority <priority>` | string | (unchanged) | New priority |
| `-T, --kind <kind>` | string | (unchanged) | New kind |
| `-a, --assignee <name>` | string | (unchanged) | New assignee |
| `--parent <id>` | string | (unchanged) | New parent issue ID |

**Examples:**

```
bmo issue edit BMO-3 --status in-progress
bmo issue edit 7 --title "Updated title" --priority high --json
bmo issue edit BMO-10 --assignee bob --parent BMO-1
```

**JSON output** (`data` field): the updated issue object.

### bmo issue move

Change an issue's status.

**Synopsis:** `bmo issue move <id> --status <status>`

| Argument/Flag | Type | Default | Description |
|---------------|------|---------|-------------|
| `<id>` | positional (required) | | Issue ID |
| `-s, --status <status>` | string | (required) | Target status |

**Examples:**

```
bmo issue move BMO-4 --status in-progress
bmo issue move 4 --status review --json
```

**JSON output** (`data` field): the updated issue object.

### bmo issue close

Mark an issue as done (sets status to `done`).

**Synopsis:** `bmo issue close <id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |

**Example:**

```
bmo issue close BMO-5
bmo issue close 5 --json
```

**JSON output** (`data` field): the updated issue object.

### bmo issue reopen

Reopen a closed issue (sets status to `todo`).

**Synopsis:** `bmo issue reopen <id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |

**Example:**

```
bmo issue reopen BMO-5
```

**JSON output** (`data` field): the updated issue object.

### bmo issue claim

Atomically claim an issue: sets status to `in-progress` and optionally records an assignee
in a single conditional SQL UPDATE. If another agent has already claimed the ticket, returns
a `conflict` error rather than overwriting.

**Synopsis:** `bmo issue claim <id> [--assignee <name>]`

| Argument/Flag | Type | Default | Description |
|---------------|------|---------|-------------|
| `<id>` | positional (required) | | Issue ID |
| `-a, --assignee <name>` | string | (none) | Assignee name to record |

**Exit codes:**

| Code | Meaning |
|------|---------|
| `0` | Claimed successfully |
| `2` | Issue not found |
| `4` | Issue is already in-progress (conflict) |

**Examples:**

```
bmo issue claim BMO-7
bmo issue claim BMO-7 --assignee alice --json
```

**JSON output (`data` field):** the updated issue object.

When the claimed issue shares file attachments with another in-progress issue, the response
includes a top-level `"file_conflicts"` key (alongside `ok`, `data`, `message`):

```json
{"ok": true, "data": <issue>, "message": "Claimed BMO-7.", "file_conflicts": [...]}
```

### bmo issue delete

Permanently delete an issue. Prompts for confirmation unless `--yes` is passed.

**Synopsis:** `bmo issue delete <id> [--yes]`

| Argument/Flag | Type | Default | Description |
|---------------|------|---------|-------------|
| `<id>` | positional (required) | | Issue ID |
| `--yes` | bool | false | Skip the confirmation prompt |

**Examples:**

```
bmo issue delete BMO-9
bmo issue delete 9 --yes --json
```

**JSON output** (`data` field): `null`.

### bmo issue log

Show the activity log for an issue.

**Synopsis:** `bmo issue log <id> [--limit <n>]`

| Argument/Flag | Type | Default | Description |
|---------------|------|---------|-------------|
| `<id>` | positional (required) | | Issue ID |
| `--limit <n>` | integer | 10 | Maximum number of log entries to show |

**Examples:**

```
bmo issue log BMO-3
bmo issue log 3 --limit 20 --json
```

**JSON output** (`data` field): array of activity entry objects.

### bmo issue graph

Show the blocking/blocked-by dependency graph for an issue.

**Synopsis:** `bmo issue graph <id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |

**Example:**

```
bmo issue graph BMO-1
bmo issue graph 1 --json
```

**JSON output** (`data` field):

```json
{
  "issue": <issue object>,
  "relations": [ <relation>, ... ]
}
```

### bmo issue comment add

Add a comment to an issue.

**Synopsis:** `bmo issue comment add <id> --body <text> [--author <name>]`

| Argument/Flag | Type | Default | Description |
|---------------|------|---------|-------------|
| `<id>` | positional (required) | | Issue ID |
| `-b, --body <text>` | string | (required) | Comment body |
| `-a, --author <name>` | string | (none) | Comment author name |

**Examples:**

```
bmo issue comment add BMO-3 --body "Investigating now"
bmo issue comment add 3 --body "Fixed in commit abc123" --author alice --json
```

**JSON output** (`data` field): the created comment object.

### bmo issue comment list

List all comments on an issue.

**Synopsis:** `bmo issue comment list <id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |

**Example:**

```
bmo issue comment list BMO-3 --json
```

**JSON output** (`data` field): array of comment objects.

### bmo issue label add

Add a label to an issue. Creates the label if it does not already exist.

**Synopsis:** `bmo issue label add <id> <name> [--color <hex>]`

| Argument/Flag | Type | Default | Description |
|---------------|------|---------|-------------|
| `<id>` | positional (required) | | Issue ID |
| `<name>` | positional (required) | | Label name |
| `--color <hex>` | string | (none) | Label color in hex format (e.g. `#ff0000`) |

**Examples:**

```
bmo issue label add BMO-2 bug
bmo issue label add BMO-2 urgent --color "#ff0000" --json
```

**JSON output** (`data` field): the label object.

### bmo issue label rm

Remove a label from an issue. The label itself is not deleted from the system.

**Synopsis:** `bmo issue label rm <id> <name>`

Alias: `bmo issue label remove`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |
| `<name>` | positional (required) | Label name |

**Example:**

```
bmo issue label rm BMO-2 bug
```

**JSON output** (`data` field): `null`.

### bmo issue label list

List all labels attached to an issue.

**Synopsis:** `bmo issue label list <id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |

**Example:**

```
bmo issue label list BMO-2 --json
```

**JSON output** (`data` field): array of label objects.

### bmo issue label delete

Delete a label from the system entirely, removing it from all issues.

**Synopsis:** `bmo issue label delete <name>`

| Argument | Type | Description |
|----------|------|-------------|
| `<name>` | positional (required) | Label name |

**Example:**

```
bmo issue label delete obsolete-label
```

**JSON output** (`data` field): `null`.

### bmo issue link add

Add a directional relationship between two issues.

**Synopsis:** `bmo issue link add <from-id> <relation> <to-id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<from-id>` | positional (required) | Source issue ID |
| `<relation>` | positional (required) | Relation kind (see enumerated values above) |
| `<to-id>` | positional (required) | Target issue ID |

**Examples:**

```
bmo issue link add BMO-1 blocks BMO-2
bmo issue link add 3 depends-on 1 --json
```

**JSON output** (`data` field): the created relation object.

### bmo issue link remove

Remove a relation by its numeric relation ID (not an issue ID).

**Synopsis:** `bmo issue link remove <relation-id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<relation-id>` | positional (required) | Numeric relation ID (obtain from `bmo issue link list`) |

**Example:**

```
bmo issue link remove 7
```

**JSON output** (`data` field): `null`.

### bmo issue link list

List all relations for an issue.

**Synopsis:** `bmo issue link list <id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |

**Example:**

```
bmo issue link list BMO-1 --json
```

**JSON output** (`data` field): array of relation objects.

### bmo issue file add

Attach a file path to an issue.

**Synopsis:** `bmo issue file add <id> <path>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |
| `<path>` | positional (required) | File path to attach |

**Example:**

```
bmo issue file add BMO-3 src/main.rs
bmo issue file add 3 docs/spec.md --json
```

**JSON output** (`data` field): the created file attachment object.

### bmo issue file rm

Remove a file attachment from an issue.

**Synopsis:** `bmo issue file rm <id> <path>`

Alias: `bmo issue file remove`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |
| `<path>` | positional (required) | File path to remove |

**Example:**

```
bmo issue file rm BMO-3 src/main.rs
```

**JSON output** (`data` field): `null`.

### bmo issue file list

List all file attachments on an issue.

**Synopsis:** `bmo issue file list <id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |

**Example:**

```
bmo issue file list BMO-3 --json
```

**JSON output** (`data` field): array of file attachment objects.

### bmo issue file conflicts

Show file conflicts: other in-progress issues that share one or more file attachments with
the given issue.

**Synopsis:** `bmo issue file conflicts <id>`

| Argument | Type | Description |
|----------|------|-------------|
| `<id>` | positional (required) | Issue ID |

Exit code is always `0` — conflict presence is information, not an error.

**Example:**

```
bmo issue file conflicts BMO-7
bmo issue file conflicts BMO-7 --json
```

**JSON output** (`data` field): array of conflict objects, empty array when no conflicts.

```json
[
  {
    "file": "src/auth.rs",
    "conflicts_with": [
      {"id": 12, "title": "Refactor auth middleware", "status": "in-progress"}
    ]
  }
]
```
