# Development

## Just Recipes

This project uses [`just`](https://github.com/casey/just) to run common tasks:

```bash
just test    # run all tests
just check   # fmt check + clippy
just fmt     # run cargo fmt
just build   # release build
just clean   # remove build artifacts
```

## Demo

`cargo run --example demo` runs a cinematic 8-act walkthrough of bmo's core features. It spawns a web server on a random port and cycles through issue creation, status transitions, comments, labels, links, and the board view.

```bash
cargo run --example demo
```
