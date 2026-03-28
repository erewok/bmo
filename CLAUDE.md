# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this project is

**BMO** is a local-first, SQLite-backed CLI issue tracker designed for AI agents. Issues are identified as `BMO-N`. No server or external services are required — everything is stored in a single `.bmo/issues.db` SQLite file.

## Commands

This project uses [`just`](https://github.com/casey/just) as a task runner and `cargo nextest` for tests.

```bash
just test                     # run all tests
just test <filter>            # run a single test or filtered subset
just fmt                      # auto-format code
just check                    # fmt check + clippy + lint (always runs before merging)
just build                    # release build
```

## Validating Source Code Changes

**Important**: all source code changes must go through the following steps!

1. `just fmt`
2. `just check`
3. `just test`

## Architecture

BMO uses a layered architecture:

1. **CLI** (`src/cli/`) — Command parsing and orchestration using `clap` derive macros. Top-level `Commands` enum dispatches to subcommands; `IssueCommands` handles the issue subcommand tree.
2. **Domain Models** (`src/model/`) — Core types: `Issue`, `Status` (Backlog→Todo→InProgress→Review→Done), `Priority`, `Kind`, `Comment`, `Label`, `Relation`.
3. **Data Access** (`src/db/`) — `Repository` trait abstracts all DB operations; `SqliteRepository` is the only implementation. Database path is resolved in priority order: `--db` flag → `BMO_DB` env var → walk up CWD to find `.bmo/issues.db`.
4. **Output** (`src/output/`) — `Printer` trait with `OutputMode::Human` (comfy-table terminal rendering) and `OutputMode::Json` (serde_json). Commands dispatch to the printer without caring about format.
5. **Web** (`src/web/`) — Axum server with SSE for real-time board updates (polls every 3s), REST API endpoints, and Minijinja templates. Starts on port 7777 by default.
6. **Planner** (`src/planner/`) — DAG analysis for execution planning. Builds a directed acyclic graph of issue dependencies and produces phased execution plans via topological sort.
7. **Config** (`src/config/`) — TOML-based configuration.

## Key design patterns

- **Repository trait** (`src/db/mod.rs`): All business logic goes through the `Repository` trait, never directly against `SqliteRepository`. This enables test doubles.
- **sea-query for SQL**: Type-safe SQL construction via `sea-query`. Schema is defined in `src/db/schema.rs` with migrations tracked in the `meta` table.
- **SQLite WAL mode**: Enabled for concurrent access; busy timeout is 5 seconds.
- **Markdown rendering**: Issue bodies and comments are rendered Markdown→HTML via `pulldown-cmark`, then sanitized with `ammonia`.

## Integration tests

Tests in `tests/` use `assert_cmd` + `tempfile` to spin up real SQLite databases. Each test creates an isolated temp directory. The `web.rs` tests use `tower` to test Axum handlers in-process without binding a port.
