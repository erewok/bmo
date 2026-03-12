# Web Interface

`bmo web` starts a local HTTP server and opens a browser window automatically. The web interface provides a read-friendly view of all issues, their statuses, comments, labels, and relationships.

Default address: `http://127.0.0.1:7777`

## Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--no-open` | false | Start the server without opening a browser window |
| `--port <n>` | `7777` | Bind to a different port |
| `--host <addr>` | `127.0.0.1` | Bind to a different host address |

## Examples

```bash
bmo web                   # start and open browser
bmo web --no-open         # start without opening browser
bmo web --port 8080       # use a different port
bmo web --host 0.0.0.0    # listen on all interfaces
```

## Live Updates

The web interface uses server-sent events (SSE) to receive live updates as issues change. The page updates automatically without a manual refresh.

The `--json` flag has no effect on this command.
