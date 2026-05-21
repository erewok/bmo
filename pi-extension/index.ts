/**
 * pi-bmo extension
 *
 * Integrates bmo (local issue tracker) natively into pi-code:
 *   - bmo_* tools: typed wrappers for every bmo CLI command
 *   - Context injection: injects board state into the system prompt
 *     when a .bmo/issues.db is found in the project
 *   - Status widget: compact board counts above the pi editor
 *   - Slash commands: /bmo-board, /bmo-ls, /bmo-show, /bmo-next,
 *                     /bmo-plan, /bmo-stats, /bmo-init, /bmo-widget
 *
 * Install (user-global):
 *   pi install git:github.com/erewok/bmo
 *
 * Or add to ~/.pi/agent/settings.json for local dev:
 *   { "extensions": ["/path/to/bmo/pi-extension"] }
 */

import { execFile } from "node:child_process";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { registerBmoTools, hasBmoProject, getBoardCounts, getNextSummary } from "./bmo-tools.ts";

export default function (pi: ExtensionAPI): void {
  // ── bmo_* tools ────────────────────────────────────────────────────────
  registerBmoTools(pi);

  // ── Context injection ──────────────────────────────────────────────────
  // When an agent turn starts and a bmo project is active, silently inject
  // the current board summary + next issues into the system prompt context.
  pi.on("before_agent_start", async (_event, ctx) => {
    if (!hasBmoProject(ctx.cwd)) return;

    const [counts, next] = await Promise.all([
      getBoardCounts(ctx.cwd),
      getNextSummary(ctx.cwd),
    ]);
    if (!counts) return;

    const total = Object.values(counts).reduce((a, b) => a + b, 0);
    if (total === 0 && !next) return;

    const statusLine = [
      `backlog:${counts.backlog}`,
      `todo:${counts.todo}`,
      `in-progress:${counts.in_progress}`,
      `review:${counts.review}`,
      `done:${counts.done}`,
    ].join("  ");

    let content = `## Active bmo Project\n\nBoard: ${statusLine}`;
    if (next) content += `\n\nNext work-ready:\n${next}`;

    return {
      message: {
        customType: "bmo-context",
        content,
        display: false,
      },
    };
  });

  // ── Status widget ──────────────────────────────────────────────────────
  // Shown above the editor when a bmo project is active.
  // Toggle with /bmo-widget. Preference persists across sessions.

  let widgetEnabled = true;

  // Restore saved preference from session entries on startup.
  pi.on("session_start", async (event, ctx) => {
    if (event.reason === "startup" || event.reason === "resume") {
      for (const entry of ctx.sessionManager.getEntries()) {
        if (entry.type === "custom" && entry.customType === "bmo-widget-pref") {
          widgetEnabled = (entry as any).data?.enabled ?? true;
        }
      }
    }
    await refreshWidget(ctx.cwd, ctx.ui);
  });

  pi.on("agent_end", async (_event, ctx) => {
    await refreshWidget(ctx.cwd, ctx.ui);
  });

  async function refreshWidget(
    cwd: string,
    ui: { setWidget(id: string, content: any): void },
  ): Promise<void> {
    if (!widgetEnabled || !hasBmoProject(cwd)) {
      ui.setWidget("bmo", undefined);
      return;
    }

    const counts = await getBoardCounts(cwd);
    if (!counts) {
      ui.setWidget("bmo", undefined);
      return;
    }

    const total = Object.values(counts).reduce((a, b) => a + b, 0);

    if (total === 0) {
      // Show a minimal presence so users know the widget is active.
      ui.setWidget("bmo", ["bmo  (no issues)"]);
      return;
    }

    // Only show non-zero counts — keeps the line compact.
    const parts: string[] = ["bmo"];
    if (counts.backlog)     parts.push(`${counts.backlog} backlog`);
    if (counts.todo)        parts.push(`${counts.todo} todo`);
    if (counts.in_progress) parts.push(`${counts.in_progress} in-progress`);
    if (counts.review)      parts.push(`${counts.review} review`);
    if (counts.done)        parts.push(`${counts.done} done`);

    ui.setWidget("bmo", [parts.join("  ")]);
  }

  // ── Slash commands ─────────────────────────────────────────────────────

  pi.registerCommand("bmo-widget", {
    description: "Toggle the bmo board status widget on or off",
    handler: async (_args, ctx) => {
      widgetEnabled = !widgetEnabled;
      pi.appendEntry("bmo-widget-pref", { enabled: widgetEnabled });
      if (widgetEnabled) {
        await refreshWidget(ctx.cwd, ctx.ui);
        ctx.ui.notify("bmo widget enabled", "info");
      } else {
        ctx.ui.setWidget("bmo", undefined);
        ctx.ui.notify("bmo widget disabled", "info");
      }
    },
  });

  pi.registerCommand("bmo-init", {
    description: "Initialize a bmo project in the current directory",
    handler: async (_args, ctx) => {
      const { stdout, stderr } = await bmoExec(["init"], ctx.cwd);
      ctx.ui.notify(stdout.trim() || stderr.trim() || "bmo initialized.", "info");
      await refreshWidget(ctx.cwd, ctx.ui);
    },
  });

  pi.registerCommand("bmo-ls", {
    description: "List bmo issues. Optionally filter by status: /bmo-ls in-progress",
    handler: async (args, ctx) => {
      if (!hasBmoProject(ctx.cwd)) {
        ctx.ui.notify("No bmo project found. Run /bmo-init first.", "warning");
        return;
      }
      const cliArgs = ["list"];
      const status = args?.trim();
      if (status) cliArgs.push("--status", status);
      const { stdout, stderr } = await bmoExec(cliArgs, ctx.cwd);
      pi.sendMessage(
        { customType: "bmo-list", content: stripAnsi(stdout || stderr || "(no output)").trim(), display: true },
        { triggerTurn: false },
      );
    },
  });

  pi.registerCommand("bmo-show", {
    description: "Show a bmo issue with comments and relations: /bmo-show BMO-7",
    handler: async (args, ctx) => {
      if (!hasBmoProject(ctx.cwd)) {
        ctx.ui.notify("No bmo project found. Run /bmo-init first.", "warning");
        return;
      }
      const id = args?.trim();
      if (!id) {
        ctx.ui.notify("Usage: /bmo-show <id>  (e.g. /bmo-show BMO-7)", "warning");
        return;
      }
      const { stdout, stderr } = await bmoExec(["show", id], ctx.cwd);
      pi.sendMessage(
        { customType: "bmo-show", content: stripAnsi(stdout || stderr || "(no output)").trim(), display: true },
        { triggerTurn: false },
      );
    },
  });

  pi.registerCommand("bmo-stats", {
    description: "Show bmo issue counts by status and priority",
    handler: async (_args, ctx) => {
      if (!hasBmoProject(ctx.cwd)) {
        ctx.ui.notify("No bmo project found. Run /bmo-init first.", "warning");
        return;
      }
      const { stdout, stderr } = await bmoExec(["stats"], ctx.cwd);
      pi.sendMessage(
        { customType: "bmo-stats", content: stripAnsi(stdout || stderr || "(no output)").trim(), display: true },
        { triggerTurn: false },
      );
    },
  });

  pi.registerCommand("bmo-board", {
    description: "Show the current bmo kanban board",
    handler: async (_args, ctx) => {
      if (!hasBmoProject(ctx.cwd)) {
        ctx.ui.notify("No bmo project found. Run /bmo-init first.", "warning");
        return;
      }
      const { stdout, stderr } = await bmoExec(["board"], ctx.cwd);
      pi.sendMessage(
        { customType: "bmo-board", content: stripAnsi(stdout || stderr || "(no output)").trim(), display: true },
        { triggerTurn: false },
      );
    },
  });

  pi.registerCommand("bmo-next", {
    description: "Show next work-ready bmo issues",
    handler: async (_args, ctx) => {
      if (!hasBmoProject(ctx.cwd)) {
        ctx.ui.notify("No bmo project found. Run /bmo-init first.", "warning");
        return;
      }
      const { stdout, stderr } = await bmoExec(["next"], ctx.cwd);
      pi.sendMessage(
        { customType: "bmo-next", content: stripAnsi(stdout || stderr || "(no output)").trim(), display: true },
        { triggerTurn: false },
      );
    },
  });

  pi.registerCommand("bmo-plan", {
    description: "Show the phased bmo execution plan",
    handler: async (_args, ctx) => {
      if (!hasBmoProject(ctx.cwd)) {
        ctx.ui.notify("No bmo project found. Run /bmo-init first.", "warning");
        return;
      }
      const { stdout, stderr } = await bmoExec(["plan"], ctx.cwd);
      pi.sendMessage(
        { customType: "bmo-plan", content: stripAnsi(stdout || stderr || "(no output)").trim(), display: true },
        { triggerTurn: false },
      );
    },
  });
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function bmoExec(args: string[], cwd: string): Promise<{ stdout: string; stderr: string }> {
  return new Promise((resolve) => {
    execFile("bmo", args, { cwd }, (_err, stdout, stderr) => resolve({ stdout, stderr }));
  });
}

/** Strip ANSI escape codes so raw bmo CLI output displays cleanly in chat. */
function stripAnsi(str: string): string {
  // eslint-disable-next-line no-control-regex
  return str.replace(/\x1b\[[0-9;]*[mGKHF]/g, "");
}
