# bmo + pi-code Integration

[pi-code](https://pi.dev) is an AI coding agent that runs in the terminal. The `pi-bmo`
extension integrates bmo natively into pi, replacing raw shell commands with typed tools
the LLM can call directly and adding board-state awareness to every session.

## What the Extension Provides

| Feature | Description |
|---------|-------------|
| **`bmo_*` tools** | Typed wrappers for every bmo command — the LLM calls structured tools instead of constructing shell strings |
| **Context injection** | On every agent turn, the current board summary and next work-ready issues are silently injected into context — no extra tool call needed |
| **Status widget** | Compact board counts (`○3 ●2 ◐1 ◎0 ✔5`) displayed above the pi editor, updated after every turn |
| **Slash commands** | `/bmo-board`, `/bmo-ls`, `/bmo-show`, `/bmo-next`, `/bmo-plan`, `/bmo-stats`, `/bmo-init`, `/bmo-widget` |

### Available Tools

| Tool | Equivalent bmo command |
|------|------------------------|
| `bmo_init` | `bmo init` |
| `bmo_agent_init` | `bmo agent-init` |
| `bmo_board` | `bmo board --json` |
| `bmo_plan` | `bmo plan --json [--phase N]` |
| `bmo_next` | `bmo next --json` |
| `bmo_show` | `bmo show <id> --json` |
| `bmo_list` | `bmo list --json [--status] [--assignee]` |
| `bmo_create` | `bmo create -t ...` |
| `bmo_edit` | `bmo edit <id> ...` |
| `bmo_move` | `bmo move <id> -s ...` |
| `bmo_claim` | `bmo claim <id> --assignee ...` |
| `bmo_close` | `bmo close <id>` |
| `bmo_reopen` | `bmo reopen <id>` |
| `bmo_comment` | `bmo comment add/list` |
| `bmo_link` | `bmo link add <a> <rel> <b>` |
| `bmo_file` | `bmo file add <id> <path>` |
| `bmo_stats` | `bmo stats` |

The extension discovers the bmo project the same way the CLI does — by walking up from the
current working directory looking for `.bmo/issues.db`. No configuration needed.

## Installation

Install the extension into pi's global configuration:

```bash
pi install git:github.com/erewok/bmo
```

pi clones the repo, runs `npm install`, and loads the extension from `pi-extension/`
automatically on next start.

To try it without making it permanent:

```bash
pi -e git:github.com/erewok/bmo
```

### Local Development

If you have the bmo repo checked out locally, add the extension path to
`~/.pi/agent/settings.json` instead:

```json
{
  "extensions": ["/path/to/bmo/pi-extension"]
}
```

## Dev-Team Extension

A companion extension, [bmo-dev-team](https://github.com/erewok/bmo-agent-setup),
builds on `pi-bmo` to provide a full AI software development team that plans and
executes work through bmo:

- **`staff-engineer`** — produces Technical Design Documents and reviews all implementation changes
- **`project-manager`** — decomposes requests into bmo issues with dependency graphs and file scoping
- **`senior-engineer`** — implements from bmo issues within their scoped files
- **`qa-engineer`** — verifies acceptance criteria and posts results as bmo comments
- **`ux-designer`** — produces UX design specs before technical planning
- **`dev-team` skill** — orchestration skill that sequences the team: design → plan → implement → review → verify

Installing `bmo-dev-team` also pulls in `pi-bmo` automatically — you do not need to
install both separately:

```bash
pi install git:github.com/erewok/bmo-agent-setup
```

### How the dev-team uses bmo

The dev-team workflow is built around bmo's dependency graph and parallel execution
model. A typical Medium-complexity feature looks like this:

1. **Staff engineer** explores the codebase and writes a TDD in `docs/tdd/`
2. **Project manager** decomposes the TDD into bmo issues with phases, dependencies,
   and file attachments — calling `bmo_create`, `bmo_link`, and `bmo_file` for each
3. **Senior engineers** run in parallel within each phase, each claiming their issue
   with `bmo_claim` and moving it to `review` with `bmo_move` on completion
4. **Staff engineer** reviews all changes and posts structured feedback as `bmo_comment`
   calls — blockers prevent the issue from closing
5. **QA engineer** verifies acceptance criteria and posts results via `bmo_comment`
6. **Orchestrator** closes each issue with `bmo_close` once review passes

Because bmo file attachments scope each issue to specific files, the orchestrator can
detect collisions before spawning parallel agents and serialize any conflicting work.

## Requirements

- [bmo](https://github.com/erewok/bmo) installed and on `$PATH`
- [pi-code](https://pi.dev) installed

The extension has no additional runtime dependencies.
