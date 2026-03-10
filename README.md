![CI](https://github.com/erewok/agent-eng-setup/actions/workflows/ci.yaml/badge.svg)

# bmo

`bmo` is a local-first, SQLite-backed CLI issue tracker for AI agents and the engineers who direct them. It stores all data in a single `.bmo/issues.db` file with no server, no network dependency, and no external services required. `bmo` is a Rust reimplementation of `docket`, using the `BMO-` issue ID prefix and adding a `bmo web` command for browser-based issue viewing.

## Installation

**From source:**

```bash
cargo install --path .
```

**Pre-built binary:**

Download the latest binary for your platform from [GitHub Releases](https://github.com/OWNER/REPO/releases).

## Quickstart

```bash
bmo init
bmo issue create --title "First issue" --priority medium --kind task
bmo issue list
bmo board
bmo web
```

## Command Reference

### Top-Level Commands

| Command | Description |
|---|---|
| `bmo init [--name <name>]` | Initialize `.bmo/issues.db` in current directory |
| `bmo config [--get <key>] [--set <key>=<value>]` | Read/write `.bmo/config.toml` |
| `bmo version` | Print version |
| `bmo stats` | Issue counts by status, priority, kind |
| `bmo export [--output <file>]` | Export all data as JSON ExportBundle |
| `bmo import <file> [--from-docket]` | Import ExportBundle; `--from-docket` remaps `DKT-` IDs |
| `bmo board [--status <s>]` | Kanban board in terminal |
| `bmo next [--assignee <a>]` | Next work-ready issue (DAG-aware) |
| `bmo plan [--assignee <a>]` | Phased execution plan |
| `bmo web [--port <n>] [--host <h>] [--no-open]` | Start local HTTP server |

### `issue` Subcommands

| Subcommand | Description |
|---|---|
| `issue create` | Create a new issue |
| `issue list` | List issues with optional filters |
| `issue show <id>` | Show full issue detail |
| `issue edit <id>` | Edit issue fields |
| `issue move <id> --status <s>` | Change issue status |
| `issue close <id>` | Close an issue (sets status to done) |
| `issue reopen <id>` | Reopen a closed issue |
| `issue delete <id> [--yes]` | Delete an issue |
| `issue comment add <id> --body <b>` | Add a comment |
| `issue comment list <id>` | List comments on an issue |
| `issue label add <id> <name>` | Add a label to an issue |
| `issue label rm <id> <name>` | Remove a label from an issue |
| `issue label list <id>` | List labels on an issue |
| `issue label delete <name>` | Delete a label globally |
| `issue link add <from> <relation> <to>` | Add a relation between issues |
| `issue link remove <link-id>` | Remove a relation |
| `issue link list <id>` | List relations for an issue |
| `issue file add <id> <path>` | Attach a file path to an issue |
| `issue file rm <id> <path>` | Remove a file attachment |
| `issue file list <id>` | List file attachments |
| `issue log <id>` | Show activity log for an issue |
| `issue graph <id>` | Show dependency graph for an issue |

Issue IDs accept both `42` and `BMO-42` everywhere.

## Migrating from docket

```bash
docket export -f export.json && bmo init && bmo import --from-docket export.json
```

## JSON Mode

Every command supports `--json` for machine-readable output:

```bash
bmo issue list --json
# {"ok": true, "data": [...], "message": "3 issue(s)"}

bmo issue show BMO-999 --json
# {"ok": false, "error": "issue 999 not found", "code": "not-found"}
```

Exit codes: `1` general · `2` not-found · `3` validation · `4` conflict

## Configuration

`.bmo/config.toml`:

```toml
project_name = "my-project"
default_assignee = "me"
web_port = 7777
web_host = "127.0.0.1"
```

Override the database path: `BMO_DB=/path/to/issues.db bmo issue list`

## Development

```bash
make test    # run all tests
make lint    # fmt check + clippy
make build   # release build
make clean   # remove build artifacts
```
