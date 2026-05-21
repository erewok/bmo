/**
 * Native bmo tools for pi-code.
 *
 * Each tool wraps a bmo CLI command with typed parameters and structured
 * JSON output, so the LLM never needs to remember raw CLI flags.
 *
 * All tools run `bmo` with cwd set to ctx.cwd, letting bmo auto-discover
 * .bmo/issues.db the same way git discovers .git — by walking up from cwd.
 */

import { execFile } from "node:child_process";
import * as path from "node:path";
import * as fs from "node:fs";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { StringEnum } from "@earendil-works/pi-ai";
import { Container, Spacer, Text } from "@earendil-works/pi-tui";
import { Type } from "typebox";

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

function runBmo(
  args: string[],
  cwd: string,
  signal?: AbortSignal,
): Promise<{ stdout: string; stderr: string; exitCode: number }> {
  return new Promise((resolve) => {
    const proc = execFile("bmo", [...args, "--json"], { cwd, shell: false }, (err, stdout, stderr) => {
      resolve({ stdout, stderr, exitCode: (err as any)?.code ?? 0 });
    });
    if (signal) {
      const kill = () => proc.kill("SIGTERM");
      if (signal.aborted) kill();
      else signal.addEventListener("abort", kill, { once: true });
    }
  });
}

function toContent(stdout: string, stderr: string, exitCode: number): { type: "text"; text: string }[] {
  if (stdout.trim()) {
    try {
      return [{ type: "text", text: JSON.stringify(JSON.parse(stdout.trim()), null, 2) }];
    } catch {
      return [{ type: "text", text: stdout.trim() }];
    }
  }
  if (stderr.trim()) return [{ type: "text", text: `Error: ${stderr.trim()}` }];
  return [{ type: "text", text: exitCode === 0 ? "(success)" : `(exit code ${exitCode})` }];
}

/** Parse the JSON response from a tool result's content text. Returns null on failure. */
function parseResult(result: { content: { type: string; text?: string }[] }): any | null {
  const block = result.content[0];
  if (!block || block.type !== "text" || !block.text) return null;
  try { return JSON.parse(block.text); } catch { return null; }
}

/** Fallback: return the raw text from a tool result. */
function rawText(result: { content: { type: string; text?: string }[] }): string {
  const block = result.content[0];
  return (block?.type === "text" && block.text) ? block.text : "(no output)";
}

const STATUS_COLOR: Record<string, "muted" | "accent" | "warning" | "text" | "success"> = {
  backlog: "muted",
  todo: "accent",
  "in-progress": "warning",
  "in_progress": "warning",
  review: "text",
  done: "success",
};

const PRIORITY_COLOR: Record<string, "error" | "warning" | "accent" | "muted" | "dim"> = {
  critical: "error",
  high: "warning",
  medium: "accent",
  low: "muted",
  none: "dim",
};

// ---------------------------------------------------------------------------
// Exported helpers (used by index.ts for widget and context injection)
// ---------------------------------------------------------------------------

/** Walk up from cwd looking for .bmo/issues.db — same discovery logic as bmo itself. */
export function hasBmoProject(cwd: string): boolean {
  let dir = cwd;
  while (true) {
    if (fs.existsSync(path.join(dir, ".bmo", "issues.db"))) return true;
    const parent = path.dirname(dir);
    if (parent === dir) return false;
    dir = parent;
  }
}

/** Return board counts by status column, or null if no bmo project or on error. */
export async function getBoardCounts(cwd: string): Promise<Record<string, number> | null> {
  if (!hasBmoProject(cwd)) return null;
  const { stdout } = await runBmo(["board"], cwd);
  try {
    const data = JSON.parse(stdout.trim());
    if (!data.ok) return null;
    const board = data.data as Record<string, unknown[]>;
    return {
      backlog: board.backlog?.length ?? 0,
      todo: board.todo?.length ?? 0,
      in_progress: board.in_progress?.length ?? 0,
      review: board.review?.length ?? 0,
      done: board.done?.length ?? 0,
    };
  } catch {
    return null;
  }
}

/** Return a compact summary of next work-ready issues, or null if none / no project. */
export async function getNextSummary(cwd: string): Promise<string | null> {
  if (!hasBmoProject(cwd)) return null;
  const { stdout } = await runBmo(["next"], cwd);
  try {
    const data = JSON.parse(stdout.trim());
    if (!data.ok || !Array.isArray(data.data) || data.data.length === 0) return null;
    return (data.data as Array<{ id: number; title: string; status: string }>)
      .map((i) => `BMO-${i.id}: ${i.title} [${i.status}]`)
      .join("\n");
  } catch {
    return null;
  }
}

// ---------------------------------------------------------------------------
// Tool registration
// ---------------------------------------------------------------------------

export function registerBmoTools(pi: ExtensionAPI): void {

  // ── bmo_init ──────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_init",
    label: "bmo init",
    description:
      "Initialize a bmo issue-tracking project in the current directory. " +
      "Creates .bmo/issues.db. Safe to call again — shows current status if already initialized.",
    promptSnippet: "Initialize or check bmo project in the current directory",
    parameters: Type.Object({}),
    async execute(_id, _p, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await new Promise<{ stdout: string; stderr: string; exitCode: number }>(
        (resolve) => {
          const proc = execFile("bmo", ["init"], { cwd: ctx.cwd, shell: false }, (err, stdout, stderr) => {
            resolve({ stdout, stderr, exitCode: (err as any)?.code ?? 0 });
          });
          if (signal) {
            const kill = () => proc.kill("SIGTERM");
            if (signal.aborted) kill();
            else signal.addEventListener("abort", kill, { once: true });
          }
        },
      );
      const text = stdout.trim() || stderr.trim() || (exitCode === 0 ? "bmo initialized." : `Exit ${exitCode}`);
      return { content: [{ type: "text", text }], details: {} };
    },
    renderResult(result, _opts, theme) {
      const text = rawText(result);
      const icon = result.isError ? theme.fg("error", "✗ ") : theme.fg("success", "✓ ");
      return new Text(icon + text, 0, 0);
    },
  });

  // ── bmo_agent_init ────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_agent_init",
    label: "bmo agent-init",
    description:
      "One-shot session orientation: initializes bmo if needed, then prints board, " +
      "next work-ready issues, stats, and a command cheat sheet. " +
      "Call this at the start of every agent session that touches bmo.",
    promptSnippet: "Orient to the current bmo project (board + next + stats + cheat sheet)",
    promptGuidelines: [
      "Use bmo_agent_init at the start of every agent session before any planning or implementation.",
    ],
    parameters: Type.Object({}),
    async execute(_id, _p, signal, _u, ctx) {
      const { stdout, stderr } = await new Promise<{ stdout: string; stderr: string }>((resolve) => {
        const proc = execFile("bmo", ["agent-init"], { cwd: ctx.cwd, shell: false }, (_err, stdout, stderr) => {
          resolve({ stdout, stderr });
        });
        if (signal) {
          const kill = () => proc.kill("SIGTERM");
          if (signal.aborted) kill();
          else signal.addEventListener("abort", kill, { once: true });
        }
      });
      return { content: [{ type: "text", text: stdout.trim() || stderr.trim() || "(no output)" }], details: {} };
    },
  });

  // ── bmo_board ─────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_board",
    label: "bmo board",
    description: "Return the full kanban board organized by status column (backlog / todo / in-progress / review / done).",
    promptSnippet: "Get the full bmo kanban board as structured JSON",
    parameters: Type.Object({}),
    async execute(_id, _p, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await runBmo(["board"], ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(_args, theme) {
      return new Text(theme.fg("toolTitle", theme.bold("bmo board")), 0, 0);
    },
    renderResult(result, _opts, theme) {
      const data = parseResult(result);
      if (!data) return new Text(theme.fg("error", rawText(result)), 0, 0);
      if (!data.ok) return new Text(theme.fg("error", data.error ?? rawText(result)), 0, 0);

      const board = data.data as Record<string, Array<{ id: number; title: string; priority: string }>>;
      const columns = [
        { key: "backlog",     label: "Backlog",      color: "muted"    },
        { key: "todo",        label: "Todo",         color: "accent"   },
        { key: "in_progress", label: "In Progress",  color: "warning"  },
        { key: "review",      label: "Review",       color: "text"     },
        { key: "done",        label: "Done",         color: "success"  },
      ] as const;

      const total = columns.reduce((n, c) => n + (board[c.key]?.length ?? 0), 0);
      if (total === 0) {
        return new Text(theme.fg("dim", "Board is empty — no issues yet."), 0, 0);
      }

      const container = new Container();
      for (const col of columns) {
        const issues = board[col.key] ?? [];
        if (issues.length === 0) continue;

        const header = theme.fg(col.color, `${col.label.toUpperCase()} (${issues.length})`);
        container.addChild(new Text(header, 0, 0));
        for (const issue of issues.slice(0, 8)) {
          const priorityMark = issue.priority === "critical" ? theme.fg("error", "!") :
                               issue.priority === "high"     ? theme.fg("warning", "↑") : " ";
          container.addChild(new Text(
            `  ${priorityMark} ${theme.fg("dim", `BMO-${issue.id}`)}  ${issue.title}`,
            0, 0,
          ));
        }
        if (issues.length > 8) {
          container.addChild(new Text(theme.fg("muted", `  … ${issues.length - 8} more`), 0, 0));
        }
        container.addChild(new Spacer(1));
      }
      return container;
    },
  });

  // ── bmo_plan ──────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_plan",
    label: "bmo plan",
    description:
      "Return the phased execution plan derived from the issue dependency graph. " +
      "Issues in the same phase can run in parallel. " +
      "Filter to a specific phase number with the optional `phase` parameter.",
    promptSnippet: "Get the phased bmo execution plan (all phases, or a specific phase)",
    parameters: Type.Object({
      phase: Type.Optional(Type.Number({ description: "Show only this phase number (1-indexed)" })),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const args = ["plan"];
      if (params.phase != null) args.push("--phase", String(params.phase));
      const { stdout, stderr, exitCode } = await runBmo(args, ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      const suffix = args.phase != null ? theme.fg("dim", ` phase ${args.phase}`) : "";
      return new Text(theme.fg("toolTitle", theme.bold("bmo plan")) + suffix, 0, 0);
    },
    renderResult(result, _opts, theme) {
      const data = parseResult(result);
      if (!data) return new Text(theme.fg("error", rawText(result)), 0, 0);
      if (!data.ok) return new Text(theme.fg("error", data.error ?? rawText(result)), 0, 0);

      const { phases, total_phases } = data.data as {
        phases: Array<{ number: number; issues: Array<{ id: number; title: string }> }>;
        total_phases: number;
      };

      if (total_phases === 0) return new Text(theme.fg("dim", "No phases — board is empty."), 0, 0);

      const container = new Container();
      for (const phase of phases) {
        container.addChild(new Text(
          theme.fg("accent", `Phase ${phase.number}`) +
          theme.fg("dim", `  (${phase.issues.length} issue${phase.issues.length !== 1 ? "s" : ""}, can run in parallel)`),
          0, 0,
        ));
        for (const issue of phase.issues) {
          container.addChild(new Text(
            `  ${theme.fg("dim", `BMO-${issue.id}`)}  ${issue.title}`,
            0, 0,
          ));
        }
        container.addChild(new Spacer(1));
      }
      return container;
    },
  });

  // ── bmo_next ──────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_next",
    label: "bmo next",
    description: "Return next work-ready issues: status todo or backlog with all blocking issues resolved.",
    promptSnippet: "Get next work-ready bmo issues (all blockers resolved)",
    parameters: Type.Object({}),
    async execute(_id, _p, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await runBmo(["next"], ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(_args, theme) {
      return new Text(theme.fg("toolTitle", theme.bold("bmo next")), 0, 0);
    },
    renderResult(result, _opts, theme) {
      const data = parseResult(result);
      if (!data) return new Text(theme.fg("error", rawText(result)), 0, 0);
      if (!data.ok) return new Text(theme.fg("error", data.error ?? rawText(result)), 0, 0);

      const issues = data.data as Array<{ id: number; title: string; priority: string; status: string }>;
      if (issues.length === 0) return new Text(theme.fg("dim", "No work-ready issues."), 0, 0);

      const container = new Container();
      container.addChild(new Text(theme.fg("muted", `${issues.length} work-ready:`), 0, 0));
      for (const issue of issues) {
        const pColor = PRIORITY_COLOR[issue.priority] ?? "dim";
        container.addChild(new Text(
          `  ${theme.fg("dim", `BMO-${issue.id}`)}  ${issue.title}  ${theme.fg(pColor, issue.priority)}`,
          0, 0,
        ));
      }
      return container;
    },
  });

  // ── bmo_show ──────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_show",
    label: "bmo show",
    description:
      "Return full details for an issue: title, description, status, priority, assignee, " +
      "comments, relations (blocks / blocked-by), sub-issues, and attached files.",
    promptSnippet: "Get full details for a bmo issue including comments and relations",
    parameters: Type.Object({
      id: Type.Union([Type.String(), Type.Number()], {
        description: "Issue ID — numeric (1) or BMO-prefixed (BMO-1)",
      }),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await runBmo(["show", String(params.id)], ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo show")) + "  " + theme.fg("accent", String(args.id)),
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      const data = parseResult(result);
      if (!data) return new Text(theme.fg("error", rawText(result)), 0, 0);
      if (!data.ok) return new Text(theme.fg("error", data.error ?? rawText(result)), 0, 0);

      const { issue, comments = [], relations = [], sub_issues = [] } = data.data as {
        issue: {
          id: number; title: string; status: string; priority: string;
          assignee: string | null; description: string; files: string[];
        };
        comments: Array<{ author: string; body: string; created_at: string }>;
        relations: Array<{ from_id: number; to_id: number; kind: string }>;
        sub_issues: Array<{ id: number; title: string; status: string }>;
      };

      const sColor = STATUS_COLOR[issue.status] ?? "muted";
      const pColor = PRIORITY_COLOR[issue.priority] ?? "dim";
      const container = new Container();

      // Title + ID
      container.addChild(new Text(
        theme.fg("accent", theme.bold(`BMO-${issue.id}`)) + "  " + theme.bold(issue.title),
        0, 0,
      ));

      // Status / priority / assignee
      const meta = [
        theme.fg(sColor, issue.status),
        theme.fg(pColor, issue.priority),
        issue.assignee ? theme.fg("dim", `→ ${issue.assignee}`) : "",
      ].filter(Boolean).join("  ");
      container.addChild(new Text(meta, 0, 0));

      // Description
      if (issue.description?.trim()) {
        container.addChild(new Spacer(1));
        const lines = issue.description.trim().split("\n").slice(0, 6);
        for (const line of lines) container.addChild(new Text(theme.fg("text", line), 0, 0));
        if (issue.description.split("\n").length > 6) {
          container.addChild(new Text(theme.fg("muted", "  …"), 0, 0));
        }
      }

      // Files
      if (issue.files?.length) {
        container.addChild(new Spacer(1));
        container.addChild(new Text(theme.fg("muted", "files:"), 0, 0));
        for (const f of issue.files) {
          container.addChild(new Text(theme.fg("dim", `  ${f}`), 0, 0));
        }
      }

      // Relations
      if (relations.length) {
        container.addChild(new Spacer(1));
        for (const rel of relations) {
          const other = rel.from_id === issue.id ? rel.to_id : rel.from_id;
          container.addChild(new Text(
            theme.fg("muted", rel.kind) + "  " + theme.fg("dim", `BMO-${other}`),
            0, 0,
          ));
        }
      }

      // Sub-issues
      if (sub_issues.length) {
        container.addChild(new Spacer(1));
        container.addChild(new Text(theme.fg("muted", `${sub_issues.length} sub-issue${sub_issues.length !== 1 ? "s" : ""}:`), 0, 0));
        for (const sub of sub_issues) {
          const sCol = STATUS_COLOR[sub.status] ?? "muted";
          container.addChild(new Text(
            `  ${theme.fg("dim", `BMO-${sub.id}`)}  ${sub.title}  ${theme.fg(sCol, sub.status)}`,
            0, 0,
          ));
        }
      }

      // Comments
      if (comments.length) {
        container.addChild(new Spacer(1));
        container.addChild(new Text(theme.fg("muted", `${comments.length} comment${comments.length !== 1 ? "s" : ""}:`), 0, 0));
        for (const c of comments.slice(-5)) {
          const preview = c.body.length > 100 ? c.body.slice(0, 100) + "…" : c.body;
          container.addChild(new Text(
            `  ${theme.fg("accent", c.author)}  ${theme.fg("text", preview)}`,
            0, 0,
          ));
        }
        if (comments.length > 5) {
          container.addChild(new Text(theme.fg("dim", `  … ${comments.length - 5} earlier`), 0, 0));
        }
      }

      return container;
    },
  });

  // ── bmo_list ──────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_list",
    label: "bmo list",
    description: "List issues with optional filters.",
    promptSnippet: "List bmo issues (optional status / assignee filter)",
    parameters: Type.Object({
      status: Type.Optional(
        StringEnum(["backlog", "todo", "in-progress", "review", "done"] as const, {
          description: "Filter by status",
        }),
      ),
      assignee: Type.Optional(Type.String({ description: "Filter by assignee name" })),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const args = ["list"];
      if (params.status) args.push("--status", params.status);
      if (params.assignee) args.push("--assignee", params.assignee);
      const { stdout, stderr, exitCode } = await runBmo(args, ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      const filters = [
        args.status ? theme.fg("accent", String(args.status)) : "",
        args.assignee ? theme.fg("dim", `@${args.assignee}`) : "",
      ].filter(Boolean).join("  ");
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo list")) + (filters ? "  " + filters : ""),
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      const data = parseResult(result);
      if (!data) return new Text(theme.fg("error", rawText(result)), 0, 0);
      if (!data.ok) return new Text(theme.fg("error", data.error ?? rawText(result)), 0, 0);

      const issues = data.data as Array<{
        id: number; title: string; status: string; priority: string; assignee: string | null;
      }>;
      if (issues.length === 0) return new Text(theme.fg("dim", "No issues found."), 0, 0);

      const container = new Container();
      container.addChild(new Text(theme.fg("muted", `${issues.length} issue${issues.length !== 1 ? "s" : ""}:`), 0, 0));
      for (const issue of issues) {
        const sColor = STATUS_COLOR[issue.status] ?? "muted";
        const pColor = PRIORITY_COLOR[issue.priority] ?? "dim";
        const assignee = issue.assignee ? theme.fg("dim", ` → ${issue.assignee}`) : "";
        container.addChild(new Text(
          `  ${theme.fg("dim", `BMO-${issue.id}`)}  ${issue.title}  ` +
          `${theme.fg(sColor, issue.status)}  ${theme.fg(pColor, issue.priority)}${assignee}`,
          0, 0,
        ));
      }
      return container;
    },
  });

  // ── bmo_create ────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_create",
    label: "bmo create",
    description:
      "Create a new bmo issue. Call bmo_file immediately after to attach all files scoped to this issue.",
    promptSnippet: "Create a new bmo issue",
    promptGuidelines: [
      "After bmo_create, immediately call bmo_file for every file scoped to the new issue.",
    ],
    parameters: Type.Object({
      title: Type.String({ description: "Issue title" }),
      description: Type.Optional(Type.String({ description: "Issue body" })),
      parent: Type.Optional(
        Type.Union([Type.String(), Type.Number()], { description: "Parent issue ID" }),
      ),
      priority: Type.Optional(
        StringEnum(["critical", "high", "medium", "low", "none"] as const, {
          description: "Priority (default: medium)",
        }),
      ),
      kind: Type.Optional(
        StringEnum(["task", "bug", "feature", "chore"] as const, {
          description: "Issue kind (default: task)",
        }),
      ),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const args = ["create", "-t", params.title];
      if (params.description) args.push("-d", params.description);
      if (params.parent != null) args.push("--parent", String(params.parent));
      if (params.priority) args.push("-p", params.priority);
      if (params.kind) args.push("-k", params.kind);
      const { stdout, stderr, exitCode } = await runBmo(args, ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      const title = String(args.title ?? "");
      const preview = title.length > 50 ? title.slice(0, 50) + "…" : title;
      const suffix = args.priority && args.priority !== "medium"
        ? "  " + theme.fg(PRIORITY_COLOR[String(args.priority)] ?? "dim", String(args.priority))
        : "";
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo create")) + "  " + theme.fg("text", preview) + suffix,
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      if (result.isError) return new Text(theme.fg("error", "✗ ") + rawText(result), 0, 0);
      // bmo create returns plain text like "Created BMO-7: title"
      return new Text(theme.fg("success", "✓ ") + rawText(result), 0, 0);
    },
  });

  // ── bmo_edit ──────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_edit",
    label: "bmo edit",
    description: "Edit one or more fields on an existing issue. Only provided fields are updated.",
    promptSnippet: "Edit bmo issue fields (title, description, status, priority, assignee, parent)",
    parameters: Type.Object({
      id: Type.Union([Type.String(), Type.Number()], { description: "Issue ID" }),
      title: Type.Optional(Type.String()),
      description: Type.Optional(Type.String()),
      status: Type.Optional(
        StringEnum(["backlog", "todo", "in-progress", "review", "done"] as const),
      ),
      priority: Type.Optional(
        StringEnum(["critical", "high", "medium", "low", "none"] as const),
      ),
      assignee: Type.Optional(Type.String({ description: "Pass empty string to clear assignee" })),
      parent: Type.Optional(Type.Union([Type.String(), Type.Number()])),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const args = ["edit", String(params.id)];
      if (params.title) args.push("-t", params.title);
      if (params.description) args.push("-d", params.description);
      if (params.status) args.push("-s", params.status);
      if (params.priority) args.push("-p", params.priority);
      if (params.assignee !== undefined) args.push("-a", params.assignee);
      if (params.parent != null) args.push("--parent", String(params.parent));
      const { stdout, stderr, exitCode } = await runBmo(args, ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      const changes: string[] = [];
      if (args.status) changes.push(theme.fg(STATUS_COLOR[String(args.status)] ?? "muted", String(args.status)));
      if (args.priority) changes.push(theme.fg(PRIORITY_COLOR[String(args.priority)] ?? "dim", String(args.priority)));
      if (args.assignee !== undefined) changes.push(theme.fg("dim", args.assignee ? `→ ${args.assignee}` : "unassigned"));
      if (args.title) changes.push(theme.fg("text", `title: "${String(args.title).slice(0, 30)}"`));
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo edit")) + "  " +
        theme.fg("accent", `BMO-${args.id}`) +
        (changes.length ? "  " + changes.join("  ") : ""),
        0, 0,
      );
    },
  });

  // ── bmo_move ──────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_move",
    label: "bmo move",
    description: "Change the status of an issue.",
    promptSnippet: "Move a bmo issue to a new status",
    parameters: Type.Object({
      id: Type.Union([Type.String(), Type.Number()], { description: "Issue ID" }),
      status: StringEnum(["backlog", "todo", "in-progress", "review", "done"] as const, {
        description: "Target status",
      }),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await runBmo(
        ["move", String(params.id), "-s", params.status],
        ctx.cwd, signal,
      );
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      const sColor = STATUS_COLOR[String(args.status)] ?? "muted";
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo move")) + "  " +
        theme.fg("accent", `BMO-${args.id}`) + "  →  " +
        theme.fg(sColor, String(args.status)),
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      if (result.isError) return new Text(theme.fg("error", "✗ ") + rawText(result), 0, 0);
      return new Text(theme.fg("success", "✓ ") + rawText(result), 0, 0);
    },
  });

  // ── bmo_claim ─────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_claim",
    label: "bmo claim",
    description:
      "Atomically claim an issue: sets status to 'in-progress' and records the assignee. " +
      "Call this before starting work on any issue to prevent duplicate effort.",
    promptSnippet: "Atomically claim a bmo issue (sets in-progress + assignee)",
    promptGuidelines: [
      "Use bmo_claim before starting implementation work. Set assignee to a unique agent reference like 'se-BMO-3-1716000000'.",
    ],
    parameters: Type.Object({
      id: Type.Union([Type.String(), Type.Number()], { description: "Issue ID" }),
      assignee: Type.String({
        description: "Unique agent reference, e.g. 'se-BMO-3-1716000000'",
      }),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await runBmo(
        ["claim", String(params.id), "--assignee", params.assignee],
        ctx.cwd, signal,
      );
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo claim")) + "  " +
        theme.fg("accent", `BMO-${args.id}`) + "  " +
        theme.fg("dim", `→ ${args.assignee}`),
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      if (result.isError) return new Text(theme.fg("error", "✗ ") + rawText(result), 0, 0);
      return new Text(theme.fg("warning", "◐ ") + rawText(result), 0, 0);
    },
  });

  // ── bmo_close ─────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_close",
    label: "bmo close",
    description:
      "Mark an issue as done. Only call after staff-engineer review passes with no open blockers.",
    promptSnippet: "Close a bmo issue (mark done)",
    promptGuidelines: [
      "Use bmo_close only after staff-engineer review has passed. Issues with open review blockers must not be closed.",
    ],
    parameters: Type.Object({
      id: Type.Union([Type.String(), Type.Number()], { description: "Issue ID" }),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await runBmo(["close", String(params.id)], ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo close")) + "  " + theme.fg("accent", `BMO-${args.id}`),
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      if (result.isError) return new Text(theme.fg("error", "✗ ") + rawText(result), 0, 0);
      return new Text(theme.fg("success", "✔ ") + rawText(result), 0, 0);
    },
  });

  // ── bmo_comment ───────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_comment",
    label: "bmo comment",
    description: "Add a comment to an issue, or list all comments on an issue.",
    promptSnippet: "Add or list comments on a bmo issue",
    parameters: Type.Object({
      action: StringEnum(["add", "list"] as const, {
        description: "'add' to post a comment, 'list' to read existing comments",
      }),
      id: Type.Union([Type.String(), Type.Number()], { description: "Issue ID" }),
      author: Type.Optional(Type.String({ description: "Comment author (required for action=add)" })),
      body: Type.Optional(Type.String({ description: "Comment text (required for action=add)" })),
    }),
    async execute(_id, params, signal, _u, ctx) {
      if (params.action === "add") {
        if (!params.author || !params.body) {
          return {
            content: [{ type: "text", text: "Error: author and body are required when action=add" }],
            details: {},
            isError: true,
          };
        }
        const { stdout, stderr, exitCode } = await runBmo(
          ["comment", "add", String(params.id), "--author", params.author, "--body", params.body],
          ctx.cwd, signal,
        );
        return { content: toContent(stdout, stderr, exitCode), details: {} };
      }
      const { stdout, stderr, exitCode } = await runBmo(["comment", "list", String(params.id)], ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      const action = String(args.action);
      const suffix = action === "add" && args.author
        ? "  " + theme.fg("dim", `by ${args.author}`)
        : "";
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo comment")) + "  " +
        theme.fg("accent", action) + "  " +
        theme.fg("dim", `BMO-${args.id}`) + suffix,
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      if (result.isError) return new Text(theme.fg("error", "✗ ") + rawText(result), 0, 0);

      const data = parseResult(result);
      // add action: plain text response
      if (!data) return new Text(theme.fg("success", "✓ ") + rawText(result), 0, 0);
      if (!data.ok) return new Text(theme.fg("error", data.error ?? rawText(result)), 0, 0);

      // list action: structured comment list
      if (Array.isArray(data.data)) {
        const comments = data.data as Array<{ author: string; body: string; created_at: string }>;
        if (comments.length === 0) return new Text(theme.fg("dim", "No comments."), 0, 0);
        const container = new Container();
        container.addChild(new Text(theme.fg("muted", `${comments.length} comment${comments.length !== 1 ? "s" : ""}:`), 0, 0));
        for (const c of comments) {
          container.addChild(new Spacer(1));
          container.addChild(new Text(theme.fg("accent", c.author), 0, 0));
          for (const line of c.body.split("\n")) {
            container.addChild(new Text(theme.fg("text", `  ${line}`), 0, 0));
          }
        }
        return container;
      }

      return new Text(theme.fg("success", "✓ ") + (data.message ?? rawText(result)), 0, 0);
    },
  });

  // ── bmo_link ──────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_link",
    label: "bmo link",
    description:
      "Add a dependency link between two issues. " +
      "'blocks' means from_id must complete before to_id. " +
      "WARNING: from_id and to_id must differ — bmo enforces a DAG and will reject cycles.",
    promptSnippet: "Link two bmo issues with a dependency relation",
    promptGuidelines: [
      "Verify from_id ≠ to_id before calling bmo_link — an issue cannot depend on itself.",
    ],
    parameters: Type.Object({
      from_id: Type.Union([Type.String(), Type.Number()], { description: "Source issue ID" }),
      relation: StringEnum(["blocks", "blocked-by", "relates-to"] as const, {
        description: "Relation type",
      }),
      to_id: Type.Union([Type.String(), Type.Number()], {
        description: "Target issue ID (must differ from from_id)",
      }),
    }),
    async execute(_id, params, signal, _u, ctx) {
      if (String(params.from_id) === String(params.to_id)) {
        return {
          content: [{ type: "text", text: "Error: from_id and to_id must differ — an issue cannot depend on itself." }],
          details: {},
          isError: true,
        };
      }
      const { stdout, stderr, exitCode } = await runBmo(
        ["link", "add", String(params.from_id), params.relation, String(params.to_id)],
        ctx.cwd, signal,
      );
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo link")) + "  " +
        theme.fg("accent", `BMO-${args.from_id}`) + "  " +
        theme.fg("muted", String(args.relation)) + "  " +
        theme.fg("accent", `BMO-${args.to_id}`),
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      if (result.isError) return new Text(theme.fg("error", "✗ ") + rawText(result), 0, 0);
      return new Text(theme.fg("success", "✓ ") + rawText(result), 0, 0);
    },
  });

  // ── bmo_stats ─────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_stats",
    label: "bmo stats",
    description: "Show issue counts broken down by status and priority.",
    promptSnippet: "Get bmo issue statistics (counts by status and priority)",
    parameters: Type.Object({}),
    async execute(_id, _p, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await runBmo(["stats"], ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(_args, theme) {
      return new Text(theme.fg("toolTitle", theme.bold("bmo stats")), 0, 0);
    },
    renderResult(result, _opts, theme) {
      const data = parseResult(result);
      if (!data) return new Text(theme.fg("error", rawText(result)), 0, 0);
      if (!data.ok) return new Text(theme.fg("error", data.error ?? rawText(result)), 0, 0);

      const stats = data.data as {
        total: number;
        by_status: Record<string, number>;
        by_priority: Record<string, number>;
      };

      const container = new Container();
      container.addChild(new Text(theme.fg("muted", `Total: ${theme.fg("text", String(stats.total))}`), 0, 0));

      if (stats.by_status) {
        container.addChild(new Spacer(1));
        container.addChild(new Text(theme.fg("muted", "By status:"), 0, 0));
        for (const [status, count] of Object.entries(stats.by_status)) {
          if (count === 0) continue;
          const color = STATUS_COLOR[status] ?? "muted";
          container.addChild(new Text(
            `  ${theme.fg(color, status.padEnd(14))}  ${theme.fg("text", String(count))}`,
            0, 0,
          ));
        }
      }

      if (stats.by_priority) {
        container.addChild(new Spacer(1));
        container.addChild(new Text(theme.fg("muted", "By priority:"), 0, 0));
        for (const [priority, count] of Object.entries(stats.by_priority)) {
          if (count === 0) continue;
          const color = PRIORITY_COLOR[priority] ?? "dim";
          container.addChild(new Text(
            `  ${theme.fg(color, priority.padEnd(10))}  ${theme.fg("text", String(count))}`,
            0, 0,
          ));
        }
      }

      return container;
    },
  });

  // ── bmo_reopen ────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_reopen",
    label: "bmo reopen",
    description: "Reopen a closed issue, setting its status back to 'todo'.",
    promptSnippet: "Reopen a closed bmo issue (sets status back to todo)",
    parameters: Type.Object({
      id: Type.Union([Type.String(), Type.Number()], { description: "Issue ID" }),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await runBmo(["reopen", String(params.id)], ctx.cwd, signal);
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo reopen")) + "  " + theme.fg("accent", `BMO-${args.id}`),
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      if (result.isError) return new Text(theme.fg("error", "✗ ") + rawText(result), 0, 0);
      return new Text(theme.fg("accent", "↺ ") + rawText(result), 0, 0);
    },
  });

  // ── bmo_file ──────────────────────────────────────────────────────────────
  pi.registerTool({
    name: "bmo_file",
    label: "bmo file",
    description:
      "Attach a file path to an issue. Call once per file immediately after bmo_create. " +
      "File attachments drive parallel-agent collision detection.",
    promptSnippet: "Attach a file path to a bmo issue (call once per file)",
    parameters: Type.Object({
      id: Type.Union([Type.String(), Type.Number()], { description: "Issue ID" }),
      path: Type.String({ description: "File path to attach (relative to project root)" }),
    }),
    async execute(_id, params, signal, _u, ctx) {
      const { stdout, stderr, exitCode } = await runBmo(
        ["file", "add", String(params.id), params.path],
        ctx.cwd, signal,
      );
      return { content: toContent(stdout, stderr, exitCode), details: {} };
    },
    renderCall(args, theme) {
      return new Text(
        theme.fg("toolTitle", theme.bold("bmo file")) + "  " +
        theme.fg("accent", `BMO-${args.id}`) + "  " +
        theme.fg("dim", String(args.path)),
        0, 0,
      );
    },
    renderResult(result, _opts, theme) {
      if (result.isError) return new Text(theme.fg("error", "✗ ") + rawText(result), 0, 0);
      return new Text(theme.fg("success", "✓ ") + rawText(result), 0, 0);
    },
  });
}
