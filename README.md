# agx

AGX (`agx`) is an agentic Unix-style CLI. It reads from `STDIN`, plans a workflow of familiar tools (such as `sort`, `uniq`, `grep`, `cut`, `tr`, `jq`), executes the pipeline, and writes results to `STDOUT`.

## Installing

Once DNS is configured for `agenix.sh`, you will be able to install or update AGX with:

```sh
curl https://agenix.sh/install.sh | sh
```

This script:
- Detects your OS and architecture.
- Downloads a prebuilt binary from GitHub Releases when available.
- Falls back to building from source with `cargo` if needed.
- Installs `agx` into a standard location (for example `/usr/local/bin` or `$HOME/.local/bin`).

As an alternative, you can install from source with Rust:

```sh
cargo install agx
```

(Until AGX is published on crates.io, you may instead use `cargo install --git https://github.com/agenix-sh/agx.git --locked agx`.)

## Basic usage

AGX behaves like a Unix filter:

```sh
cat input.txt | agx "remove duplicates" > out.txt
```

For more examples, see `EXAMPLES.md`.
