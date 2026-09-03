# bmo

![bmo logo](https://raw.githubusercontent.com/erewok/bmo/main/assets/bmo-full.png)

![CI](https://github.com/erewok/bmo/actions/workflows/ci.yaml/badge.svg)
![crates.io](https://img.shields.io/crates/v/bmo)
![docs.rs](https://img.shields.io/docsrs/bmo)

`bmo` is a local-first command-line issue tracker backed by a single SQLite file, designed for use by both human developers and AI agents operating in a terminal. It requires no server, no network dependency, and no external services. Issues are identified by `BMO-N` IDs.

For an example of agents and skills that have been instructed to use `bmo` check out the [bmo-agent-setup project](https://github.com/erewok/bmo-agent-setup) (no need to run the program: the agents and skills are plain markdown files).

## Attribution

`bmo` was inspired by and adapted from [docket](https://github.com/ALT-F4-LLC/docket), an issue tracker for AI agents written by **ALT-F4-LLC**.

In addition, the code in this repository was written by [Claude Code](https://claude.ai/claude-code). The repo owner directed this work.

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

## Quickstart

```bash
bmo init
bmo create --title "First issue" --priority medium --kind task
bmo list
bmo board
bmo web
```

## pi-code Integration

If you use [pi-code](https://pi.dev), the `pi-bmo` extension integrates bmo natively
into pi. This extension adds typed `bmo_*` tools that the LLM calls directly, in addition to a live board status widget, board
state injected into every agent turn's context, and `/board` `/next` `/plan` slash commands.

```bash
pi install git:github.com/erewok/pi-bmo
```

A companion pi-code extension, [bmo-dev-team](https://github.com/erewok/bmo-agent-setup), adds a
full software development team of agents (staff-engineer, project-manager, senior-engineer,
qa-engineer, ux-designer) that plans and executes work through `bmo`.

```bash
pi install git:github.com/erewok/bmo-agent-setup
```

See [docs/pi-extension.md](docs/pi-extension.md) for the full integration guide.

## Documentation

- [docs/commands.md](docs/commands.md) — Complete command reference
- [docs/agents.md](docs/agents.md) — Integration guide for AI agents
- [docs/pi-extension.md](docs/pi-extension.md) — pi-code extension and dev-team suite
- [docs/web.md](docs/web.md) — Web interface
- [docs/data.md](docs/data.md) — Data storage and database location
- [docs/development.md](docs/development.md) — Building and running tests

## License

Apache-2.0. See [LICENSE](LICENSE) file.

`bmo` was inspired by [docket](https://github.com/ALT-F4-LLC/docket) by **ALT-F4-LLC**. Please credit that project for the ideas in this project.
