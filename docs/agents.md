# bmo for AI Agents

`bmo` is built for agent-driven workflows. This page documents the recommended integration pattern.

## Session Initialization

At the start of every session, run:

```bash
bmo init          # idempotent — safe to run every session
bmo config        # verify project settings
bmo board --json  # Kanban overview of all issues by status
bmo next --json   # work-ready issues sorted by dependency order
bmo stats         # summary of issue counts by status
```

## Finding Work

`bmo next --json` returns issues that have no unresolved blocking dependencies and are not yet done, sorted by priority.

```bash
bmo next --json
bmo issue show BMO-7 --json
bmo issue comment list BMO-7   # always check comments — they may supersede the description
bmo issue file list BMO-7      # check attached files before starting
```

## Creating Issues

Use `-d` to provide a rich description so any agent or human reading the issue later has full context:

```bash
bmo issue create -t "Implement retry logic" \
  -d "Add exponential backoff to the HTTP client. Max 3 retries. See src/client.rs." \
  -p high -T task
```

Attach all files the issue affects immediately after creation:

```bash
bmo issue file add BMO-7 src/client.rs
bmo issue file add BMO-7 src/client_test.rs
```

## Tracking Progress

Move issues through the workflow as work proceeds:

```bash
bmo issue move BMO-7 --status in-progress
bmo issue move BMO-7 --status done
# or equivalently:
bmo issue close BMO-7
```

## Adding Context via Comments

Comment on issues to record findings, decisions, and discoveries:

```bash
bmo issue comment add BMO-7 --body "Completed: rewrote retry loop. Added unit tests in src/client_test.rs."
bmo issue comment add BMO-7 --body "Discovered: the HTTP client also needs connection timeout handling. Needs a follow-up issue."
```

Comments are the canonical record of what happened. Always read them before starting work.

## Attaching Files

Record which files are relevant to an issue for traceability and to enable collision detection between concurrent agents:

```bash
bmo issue file add BMO-7 src/client.rs
bmo issue file list BMO-7
```

## Reading JSON Output

Always pass `--json` when output will be consumed by code:

```bash
bmo issue show BMO-7 --json
bmo issue list --status in-progress --json
bmo next --json
bmo board --json
```

The response envelope is always:

```json
{"ok": true, "data": <payload>, "message": "<human summary>"}
```

On error:

```json
{"ok": false, "error": "<description>", "code": "<code>"}
```

Access nested fields as `data.data.field` in JavaScript (double `.data`).
