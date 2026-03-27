# bmo for AI Agents

`bmo` is built for agent-driven workflows. This page documents the recommended integration pattern.

## Agents and Skills Coached to Use BMO

For an example of agents and skills that have been instructed to use `bmo` check out the [bmo-agent-setup project](https://github.com/erewok/bmo-agent-setup) (no need to run the program: the agents and skills are plain markdown files).

The sections below includes the kinds of commands we use in an agent or skill definition to prime the agents to use BMO for communication, work handoffs, or feedback on work.

The commands below can be included for an agent definition (for a more thorough example see the [bmo-agent-setup project](https://github.com/erewok/bmo-agent-setup)).

## Session Initialization

Run a single command at the start of every session to orient yourself:

```bash
bmo agent-init         # init + config + board + next + stats + cheat sheet — all at once
bmo agent-init --json  # same, as a single JSON envelope ready for jq
```

This is the canonical one-shot session setup. It is idempotent and safe to run every session.

**Manual equivalent** (five separate commands, for reference):

```bash
bmo init          # idempotent — safe to run every session
bmo config        # verify project settings
bmo board --json  # Kanban overview of all issues by status
bmo next --json   # work-ready issues sorted by dependency order
bmo stats         # summary of issue counts by status
```

## Coordinator Workflow: Dispatching by Phase

When acting as a coordinator, use `bmo plan` to iterate over phases and dispatch work to sub-agents:

```bash
# The plan output shows the phases.
❯ bmo plan --json
{
  "data": {
    "max_parallelism": 0,
    "phases": [],
    "total_issues": 0,
    "total_phases": 0
  },
  "message": "0 phases",
  "ok": true
}

# Get all issues ready in phase 1 as a flat array
bmo plan --phase 1 --json | jq '.data[] | {id, title}'

# Dispatch phase 1 issues to worker agents
bmo plan --phase 1 --json | jq -r '.data[].id' | while read id; do
    echo "Dispatching BMO-${id} to a worker agent..."
    # your dispatch logic here
done

# After phase 1 completes, advance to phase 2
bmo plan --phase 2 --json | jq '.data[].title'
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

Use `bmo issue claim` to atomically take ownership of an issue. It sets the status to
`in-progress` and optionally records an assignee in a single operation. If another agent
has already claimed the ticket, it returns a conflict error (exit code 4) rather than
overwriting — making it safe for concurrent multi-agent workflows.

```bash
bmo issue claim BMO-7                        # atomic: sets in-progress, fails if already claimed
bmo issue claim BMO-7 --assignee alice --json
```

`bmo issue claim` replaces the older two-step `move + edit` pattern. The old pattern still
works but `claim` is preferred when multiple agents may be picking up work simultaneously.

If the claim response includes a `"file_conflicts"` key, another in-progress issue shares
the same file attachments. Check for conflicts before beginning implementation:

```bash
bmo issue claim BMO-7 --json | jq '.file_conflicts'
bmo issue file conflicts BMO-7 --json        # also callable independently
```

Once work is complete, close the issue:

```bash
bmo issue move BMO-7 --status done
# or equivalently:
bmo issue close BMO-7
```

## Adding Context via Comments

Comment on issues to record findings, decisions, and discoveries. Use a structured tag prefix
so other agents can scan comments efficiently:

| Tag | Used by | Meaning |
|-----|---------|---------|
| `BLOCKER:` | any | Work cannot proceed without resolution |
| `CONCERN:` | staff-engineer, ux-designer | Should be addressed; not a hard stop |
| `SUGGESTION:` | staff-engineer | Optional improvement |
| `APPROVED:` | staff-engineer | Review complete, change accepted |
| `BUG:` | qa-engineer | Defect found; include reproduction steps |
| `VERIFIED:` | qa-engineer | Acceptance criteria confirmed passing |
| `FINDING:` | senior-engineer | Information discovered during implementation |
| `DECISION:` | senior-engineer | Approach chosen and rationale |
| `HANDOFF:` | any | Work complete, context for the next agent |

```bash
bmo issue comment add BMO-7 --body "FINDING: the HTTP client also needs connection timeout handling. Needs a follow-up issue."
bmo issue comment add BMO-7 --body "DECISION: used exponential backoff with jitter rather than fixed intervals."
bmo issue comment add BMO-7 --body "HANDOFF: retry logic complete. Tests in src/client_test.rs. Next agent should add integration test."
```

Scan comments by tag before starting work:

```bash
bmo issue comment list BMO-7 --json | jq '.data[] | select(.body | startswith("HANDOFF:")) | .body'
bmo issue comment list BMO-7 --json | jq '.data[] | select(.body | startswith("BLOCKER:")) | .body'
```

Comments are the canonical record of what happened. Always read them before starting work.

## Attaching Files

Record which files are relevant to an issue for traceability and to enable collision detection between concurrent agents:

```bash
bmo issue file add BMO-7 src/client.rs
bmo issue file list BMO-7
bmo issue file conflicts BMO-7 --json   # check for overlaps with other in-progress work
```

## Reading JSON Output

Always pass `--json` when output will be consumed by code. Prefer `jq` for parsing; fall back
to Python if `jq` is unavailable.

```bash
# Preferred: jq
bmo next --json | jq '.data[] | {id: .id, title: .title}'
bmo issue show BMO-7 --json | jq '.data.issue.status'
bmo board --json | jq '.data.in_progress[].id'
bmo plan --phase 1 --json | jq '.data[].id'

# Fallback: Python
bmo next --json | python3 -c "import sys,json; d=json.load(sys.stdin); print(d['data'])"
```

The standard response envelope is:

```json
{"ok": true, "data": <payload>, "message": "<human summary>"}
```

On error:

```json
{"ok": false, "error": "<description>", "code": "<code>"}
```

After parsing the response, access fields as `resp.data.field` (e.g., `resp.data.issue.title`).

The `bmo agent-init --json` envelope has an additional top-level `cheat_sheet` field and its
`data` object is keyed by sub-command:

```json
{
  "ok": true,
  "data": {
    "init":   { "db_path": "...", "already_existed": true },
    "config": { "project_name": null, "default_assignee": null, "web_port": 7777, "web_host": "127.0.0.1" },
    "board":  { "backlog": [], "todo": [], "in_progress": [], "review": [], "done": [] },
    "next":   [],
    "stats":  {}
  },
  "message": "Session initialized.",
  "cheat_sheet": "..."
}
```
