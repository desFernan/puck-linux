# Puck for Linux

> Language: **English** (here) · [한국어](README.ko.md)

> Puck currently exists on macOS — see
> [desFernan/puck-mac](https://github.com/desFernan/puck-mac) for the full
> app. This repo is the Linux port: the pet overlay and a terminal chat
> agent are here; the `PuckClient`-equivalent window and the socket bridge
> between the two are follow-up work — see Status below.
>
> Platforms: [macOS](https://github.com/desFernan/puck-mac) · [Windows](https://github.com/desFernan/puck-windows) · **Linux** (here)

### 💬 [Join the Discord](https://discord.gg/ePBZVnwSYE)

Bugs, feature requests, build help, or just want to hang out — the
[support server](https://discord.gg/ePBZVnwSYE) is the fastest way to reach
us. Come say hi!

## Status

Two pieces are here so far:

- **The pet overlay** (`puck-linux`): an always-on-top, transparent,
  animated character you can drag around, using the same avatar folder
  format as [puck-mac](https://github.com/desFernan/puck-mac).
- **The agent core** (`puck-agent`): a terminal chat client talking
  directly to the Anthropic API, with a `run_shell` tool gated behind
  per-call approval.

The two don't talk to each other yet — no socket bridge, no shared
process. The `PuckClient`-equivalent window (chat UI, code editor,
terminal pane) is also still follow-up work; `puck-agent` is a plain
terminal REPL standing in for it for now.

### Build and run — pet overlay

Requires Rust and GTK4 development headers (`libgtk-4-dev libx11-dev` on
Debian/Ubuntu, `gtk4-devel` + X11 devel packages on Fedora) on an X11
session — Wayland isn't supported yet.

```sh
cargo run --bin puck-linux -- /path/to/avatar-folder
```

An avatar folder needs a `manifest.json` and a PNG per clip — see
[puck-mac's README](https://github.com/desFernan/puck-mac#a-character) for
the manifest schema. This port reads `schema_version`, `name`, `type`,
`hitbox`, and `clips`; `idle` is the only required clip, and `walk`/`fall`/
`land` are used if present (falling back to `idle` otherwise).

### Build and run — agent

```sh
export ANTHROPIC_API_KEY=sk-ant-...   # or put it in a .env file
cargo run --bin puck-agent
```

Reads `ANTHROPIC_API_KEY` from the environment, or from a `.env` file
(`KEY=VALUE` per line) in the current directory if the environment
variable isn't already set — matching puck-mac's credential file. Defaults
to `claude-opus-5`; override with `PUCK_AGENT_MODEL`.

The only tool right now is `run_shell`, which runs a shell command with the
same permissions as the `puck-agent` process — it is **not** sandboxed or
allowlisted. Every call is shown to you (tool name + exact input) and
requires a `y`/`yes` confirmation before it runs; anything else declines it.

### Test

```sh
cargo test --bin puck-linux   # pet overlay: parsing, animation/physics state machine
cargo test --lib              # agent: wire format, tool-call loop, a real HTTP round trip against a local mock server
```

(Not `cargo test --lib --bin puck-linux` together as one invocation's `--lib`
— they're separate targets; `cargo test` with no flags runs both plus the
`puck-agent` binary's own, currently-empty, test target.)

### Making it your own

The avatar package format (`schema_version: 1`, `manifest.json` + clip PNGs)
is defined by puck-mac and read as-is here — an avatar folder built on
macOS should drop into Linux unchanged, same as it does on Windows today.
Field reference:
[puck-mac's README](https://github.com/desFernan/puck-mac#a-character).

## Community

Want to help plan the Linux port, or just curious about progress — join us on
**[Discord](https://discord.gg/ePBZVnwSYE)**.
