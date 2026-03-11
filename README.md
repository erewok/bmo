# bmo

![CI](https://github.com/erewok/bmo/actions/workflows/ci.yaml/badge.svg)

![bmo logo](https://raw.githubusercontent.com/erewok/bmo/main/assets/bmo-full.png)

`bmo` is a local-first command-line issue tracker backed by a single SQLite file, designed for use by both human developers and AI agents operating in a terminal. It requires no server, no network dependency, and no external services. Issues are identified by `BMO-N` IDs.

## Attribution

`bmo` was inspired by and adapted from [docket](https://github.com/ALT-F4-LLC/docket), an issue tracker for AI agents written by **ALT-F4-LLC**. The design, data model, and command structure owe a direct debt to that project, and all credit for the underlying ideas belongs there.

The code in this repository was written by [Claude Code](https://claude.ai/claude-code), Anthropic's AI coding assistant. The project owner directed the work and owns the repository.

## Installation

**From crates.io (recommended):**

```bash
cargo install bmo
```

**From source:**

```bash
cargo install --path .
```

**Pre-built binaries:**

Download the latest binary for your platform from [GitHub Releases](https://github.com/erewok/bmo/releases).

**Homebrew (planned, not yet available):**

```bash
brew install erewok/tap/bmo
```

## Quickstart

```bash
bmo init
bmo issue create --title "First issue" --priority medium --kind task
bmo issue list
bmo board
bmo web
```

## Commands

### Global Flags

These flags are accepted by every command.

| Flag | Type | Default | Description |
|---|---|---|---|
| `--json` | bool | false | Output results as JSON |
| `--db <PATH>` | string | auto | Override database path (also reads `BMO_DB` env var) |

---

### Issue Management

#### bmo issue create

Create a new issue.

```sh
bmo issue create --title <title> [options]
```

| Flag | Short | Type | Default | Description |
|---|---|---|---|---|
| `--title` | `-t` | string | required | Issue title |
| `--description` | `-d` | string | `""` | Issue description |
| `--status` | `-s` | string | `backlog` | Initial status |
| `--priority` | `-p` | string | `medium` | Priority level |
| `--kind` | `-T` | string | `task` | Issue kind |
| `--assignee` | `-a` | string | none | Assignee name |
| `--parent` | | string | none | Parent issue ID |
| `--label` | `-l` | string | none | Label name (repeatable) |
| `--file` | `-f` | string | none | File path to attach (repeatable) |

#### bmo issue list

List issues with optional filters.

```sh
bmo issue list [options]
```

| Flag | Short | Type | Default | Description |
|---|---|---|---|---|
| `--status` | `-s` | string | none | Filter by status (repeatable) |
| `--priority` | `-p` | string | none | Filter by priority (repeatable) |
| `--kind` | `-T` | string | none | Filter by kind (repeatable) |
| `--assignee` | `-a` | string | none | Filter by assignee |
| `--label` | `-l` | string | none | Filter by label, AND semantics (repeatable) |
| `--parent` | | string | none | Filter by parent ID |
| `--search` | | string | none | Search in title and description |
| `--limit` | | integer | `50` | Maximum number of results |
| `--sort` | | string | none | Sort field |
| `--all` | | bool | false | Include done issues |

#### bmo issue show

Show full detail for a single issue.

```sh
bmo issue show <id>
```

#### bmo issue edit

Edit issue fields. Only supplied flags are updated.

```sh
bmo issue edit <id> [options]
```

| Flag | Short | Type | Description |
|---|---|---|---|
| `--title` | `-t` | string | New title |
| `--description` | `-d` | string | New description |
| `--status` | `-s` | string | New status |
| `--priority` | `-p` | string | New priority |
| `--kind` | `-T` | string | New kind |
| `--assignee` | `-a` | string | New assignee |
| `--parent` | | string | New parent issue ID |

#### bmo issue move

Change the status of an issue.

```sh
bmo issue move <id> --status <status>
```

| Flag | Short | Type | Description |
|---|---|---|---|
| `--status` | `-s` | string | New status value |

#### bmo issue close

Close an issue by setting its status to `done`.

```sh
bmo issue close <id>
```

#### bmo issue reopen

Reopen a closed issue.

```sh
bmo issue reopen <id>
```

#### bmo issue delete

Delete an issue permanently.

```sh
bmo issue delete <id> [--yes]
```

| Flag | Type | Description |
|---|---|---|
| `--yes` | bool | Skip confirmation prompt |

#### bmo issue comment add

Add a comment to an issue.

```sh
bmo issue comment add <id> --body <text> [--author <name>]
```

| Flag | Short | Type | Description |
|---|---|---|---|
| `--body` | `-b` | string | Comment text |
| `--author` | `-a` | string | Comment author name |

#### bmo issue comment list

List all comments on an issue.

```sh
bmo issue comment list <id>
```

#### bmo issue label add

Add a label to an issue. Creates the label if it does not exist.

```sh
bmo issue label add <id> <name> [--color <hex>]
```

| Flag | Type | Description |
|---|---|---|
| `--color` | string | Label color as a hex string (e.g. `#ff0000`) |

#### bmo issue label rm

Remove a label from an issue. Accepts `rm` or `remove`.

```sh
bmo issue label rm <id> <name>
```

#### bmo issue label list

List all labels attached to an issue.

```sh
bmo issue label list <id>
```

#### bmo issue label delete

Delete a label globally, removing it from all issues.

```sh
bmo issue label delete <name>
```

#### bmo issue link add

Add a directional relation between two issues.

```sh
bmo issue link add <from-id> <relation> <to-id>
```

Valid relation values: `blocks`, `blocked-by`, `depends-on`, `dependency-of`, `relates-to`, `duplicates`, `duplicate-of`.

#### bmo issue link remove

Remove a relation by its numeric relation ID.

```sh
bmo issue link remove <link-id>
```

#### bmo issue link list

List all relations for an issue.

```sh
bmo issue link list <id>
```

#### bmo issue file add

Attach a file path to an issue.

```sh
bmo issue file add <id> <path>
```

#### bmo issue file rm

Remove a file attachment from an issue. Accepts `rm` or `remove`.

```sh
bmo issue file rm <id> <path>
```

#### bmo issue file list

List all file attachments for an issue.

```sh
bmo issue file list <id>
```

#### bmo issue log

Show the activity log for an issue.

```sh
bmo issue log <id> [--limit <n>]
```

| Flag | Type | Default | Description |
|---|---|---|---|
| `--limit` | integer | `10` | Maximum number of log entries to show |

#### bmo issue graph

Show the dependency graph for an issue.

```sh
bmo issue graph <id>
```

Issue IDs accept both `42` and `BMO-42` everywhere.

---

### Board and Planning

#### bmo board

Show a Kanban board of all issues grouped by status.

```sh
bmo board [options]
```

| Flag | Short | Type | Description |
|---|---|---|---|
| `--label` | `-l` | string | Filter by label (repeatable) |
| `--priority` | `-p` | string | Filter by priority (repeatable) |
| `--assignee` | `-a` | string | Filter by assignee |

#### bmo next

Show the next work-ready issues. An issue is work-ready when it has no unresolved blocking dependencies.

```sh
bmo next [options]
```

| Flag | Short | Type | Default | Description |
|---|---|---|---|---|
| `--assignee` | `-a` | string | none | Filter by assignee |
| `--limit` | | integer | `10` | Maximum number of results |

#### bmo plan

Show a phased execution plan ordered by dependency relationships.

```sh
bmo plan [options]
```

| Flag | Short | Type | Description |
|---|---|---|---|
| `--assignee` | `-a` | string | Filter by assignee |

---

### Statistics and Export

#### bmo stats

Show issue counts grouped by status, priority, and kind.

```sh
bmo stats
```

#### bmo export

Export all issues, comments, labels, relations, activity, and file attachments as a JSON bundle.

```
bmo export [--output <file>]
```

| Flag | Short | Type | Default | Description |
|---|---|---|---|---|
| `--output` | `-o` | string | stdout | Output file path |

#### bmo import

Import a JSON export bundle. Use `--from-docket` when importing from a docket export.

```
bmo import <file> [--from-docket]
```

| Flag | Type | Description |
|---|---|---|
| `--from-docket` | bool | Remap `DKT-` IDs to `BMO-` IDs during import |

---

### Web Interface

#### bmo web

See [Web Interface](#web-interface) for full details.

---

### Configuration

#### bmo init

Initialize a new bmo project in the current directory. Creates `.bmo/issues.db` and `.bmo/config.toml`. Safe to run on an existing project; it is idempotent.

```
bmo init [--name <project-name>]
```

| Flag | Type | Description |
|---|---|---|
| `--name` | string | Project name written to config |

#### bmo config

Read or write project configuration. With no flags, prints all current values.

```
bmo config [--get <key>] [--set <key>=<value>]
```

| Flag | Type | Description |
|---|---|---|
| `--get <key>` | string | Print the value of a single key |
| `--set <key>=<value>` | string | Set a key to a value |

Valid keys: `project_name`, `default_assignee`, `web_port`, `web_host`.

#### bmo version

Print the current bmo version.

```
bmo version
```

---

## JSON Output

Every command accepts `--json` and returns a consistent envelope.

Success:

```json
{"ok": true, "data": <payload>, "message": "<human summary>"}
```

Error:

```json
{"ok": false, "error": "<description>", "code": "<code>"}
```

Error codes and exit statuses:

| Code | Exit | Meaning |
|---|---|---|
| `general` | 1 | Unclassified error |
| `not-found` | 2 | Requested resource does not exist |
| `validation` | 3 | Invalid input |
| `conflict` | 4 | State conflict (e.g. duplicate relation) |

The `--json` flag is designed for programmatic consumption by AI agents and other tools. All structured data in the `data` field is stable across patch releases within a major version.

---

## Enumerations

Valid string values for enumerated fields. All values are case-insensitive. Agents should use these exact strings to construct valid commands without trial and error.

**Status:** `backlog`, `todo`, `in-progress`, `review`, `done`

`in-progress` also accepts `in_progress` and `inprogress` as aliases.

**Priority:** `none`, `low`, `medium`, `high`, `critical`

**Kind:** `bug`, `feature`, `task`, `epic`, `chore`

**Relation kinds:** `blocks`, `blocked-by`, `depends-on`, `dependency-of`, `relates-to`, `duplicates`, `duplicate-of`

---

## For AI Agents

`bmo` is built for agent-driven workflows. This section documents the recommended integration pattern.

**Session initialization.** At the start of a session, run:

```bash
bmo init          # idempotent — safe to run every session
bmo board --json  # get a Kanban overview of all issues
bmo next --json   # get work-ready issues sorted by dependency order
```

**Finding work.** Use `bmo next --json` to retrieve the issues an agent should act on next. Issues returned by `bmo next` have no unresolved blocking dependencies and are not yet done.

**Creating issues.** Use `-d` to provide a rich description so that any agent or human reading the issue later has full context:

```bash
bmo issue create -t "Implement retry logic" -d "Add exponential backoff to the HTTP client. Max 3 retries. See src/client.rs." -p high -T task
```

**Tracking progress.** Move issues through the workflow as work proceeds:

```bash
bmo issue move BMO-7 --status in-progress
bmo issue move BMO-7 --status done
# or equivalently:
bmo issue close BMO-7
```

**Adding context.** Comment on issues to record findings, decisions, and discoveries:

```bash
bmo issue comment add BMO-7 --body "Completed: rewrote retry loop. Added unit tests in src/client_test.rs."
```

**Attaching files.** Record which files are relevant to an issue for traceability and collision detection:

```bash
bmo issue file add BMO-7 src/client.rs
```

**Reading machine-readable output.** Always pass `--json` when the output will be consumed by code:

```bash
bmo issue show BMO-7 --json
bmo issue list --status in-progress --json
bmo next --json
```

---

## Web Interface

`bmo web` starts a local HTTP server at `http://127.0.0.1:7777` by default and opens a browser window automatically. The web interface provides a read-friendly view of all issues, their statuses, comments, labels, and relationships.

To start without opening a browser:

```bash
bmo web --no-open
```

To bind to a different port:

```bash
bmo web --port 8080
```

The web interface is read-only. All mutations go through the CLI.

---

## Data Storage

All data is stored in `.bmo/issues.db` in the directory where `bmo init` was run. `bmo` walks up the directory tree from the current working directory to find the nearest `.bmo/` directory, so you can run `bmo` commands from any subdirectory of your project.

To use a database at an explicit path, set the `BMO_DB` environment variable or pass `--db <path>`:

```bash
BMO_DB=/path/to/issues.db bmo issue list
bmo --db /path/to/issues.db issue list
```

---

## Configuration

`.bmo/config.toml` holds project-level settings:

```toml
project_name = "my-project"
default_assignee = "me"
web_port = 7777
web_host = "127.0.0.1"
```

Read and write these values with `bmo config`:

```bash
bmo config                          # print all values
bmo config --get project_name
bmo config --set default_assignee=alice
```

---

## Migration from docket

See [docs/migration-from-docket.md](https://github.com/erewok/bmo/blob/main/docs/migration-from-docket.md) for the full migration guide.

Quick start:

```bash
docket export -f export.json && bmo init && bmo import --from-docket export.json
```

---

## Development

We use [`just`](https://github.com/casey/just) to run common recipes for this project:

```bash
just test    # run all tests
just check   # fmt check + clippy
just fmt     # run cargo fmt
just build   # release build
just clean   # remove build artifacts
```

---

## License

MIT License. See [LICENSE](LICENSE) file.

`bmo` was inspired by [docket](https://github.com/ALT-F4-LLC/docket) by **ALT-F4-LLC**. Please credit that project for the ideas in this project.
