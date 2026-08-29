# Puck for Linux

> Language: **English** (here) · [한국어](README.ko.md)

> Puck currently exists on macOS — see
> [desFernan/puck-mac](https://github.com/desFernan/puck-mac) for the full
> app. This repo is the Linux port: the pet overlay, the agent core, a
> minimal `PuckClient`-equivalent GUI, and a socket bridge connecting them
> are all here — see Status below for what's still thinner than puck-mac.
>
> Platforms: [macOS](https://github.com/desFernan/puck-mac) · [Windows](https://github.com/desFernan/puck-windows) · **Linux** (here)

### 💬 [Join the Discord](https://discord.gg/ePBZVnwSYE)

Bugs, feature requests, build help, or just want to hang out — the
[support server](https://discord.gg/ePBZVnwSYE) is the fastest way to reach
us. Come say hi!

## Status

Four pieces are here so far:

- **The pet overlay** (`puck-linux`): an always-on-top, transparent,
  animated character you can drag around, using the same avatar folder
  format as [puck-mac](https://github.com/desFernan/puck-mac).
- **The agent core** (`src/agent/`): talks directly to the Anthropic API,
  with a `run_shell` tool gated behind per-call approval.
- **Two front ends for the agent**: `puck-agent`, a terminal REPL, and
  `puck-client`, a minimal GTK4 chat window (the current
  `PuckClient`-equivalent) — approvals show as a Yes/No dialog instead of a
  terminal prompt. Neither has the code editor, terminal pane, or
  workspaces puck-mac's real `PuckClient` has.
- **A socket bridge** (`src/bridge.rs`) connecting the pieces above: while
  either front end is working on a request it tells the pet overlay to
  show a "thinking" clip, then "happy" or "sad" depending on how the turn
  ended — the pet falls back to `idle` if the loaded avatar doesn't define
  that clip. This is the first slice of puck-mac's pet-talks-to-client
  architecture; only this one message exists so far, nothing richer yet
  (no shared sessions, no forwarding chat into the pet, etc.).

Not yet ported: Wayland support (X11 only, by design for this slice), and
`PuckClient`'s code editor, terminal pane, and workspaces.

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
cargo run --bin puck-agent    # terminal chat
cargo run --bin puck-client   # GTK4 chat window (needs an X11 session)
```

Both read `ANTHROPIC_API_KEY` from the environment, or from a `.env` file
(`KEY=VALUE` per line) in the current directory if the environment
variable isn't already set — matching puck-mac's credential file. Both
default to `claude-opus-5`; override with `PUCK_AGENT_MODEL`.

The only tool right now is `run_shell`, which runs a shell command with the
same permissions as the agent process — it is **not** sandboxed or
allowlisted. Every call requires your approval before it runs, showing the
tool name and exact input: a `y`/`yes` prompt in `puck-agent`, a Yes/No
dialog in `puck-client`.

### Test

```sh
cargo test --bin puck-linux   # pet overlay: parsing, animation/physics state machine, emotion override
cargo test --lib              # agent + bridge: wire format, tool-call loop, real HTTP/socket round trips
```

(Not `cargo test --lib --bin puck-linux` together as one invocation's `--lib`
— they're separate targets; `cargo test` with no flags runs both plus the
`puck-agent`/`puck-client` binaries' own, currently-empty, test targets.)

Bridge tests use a real Unix socket at a temp path — set `PUCK_BRIDGE_SOCKET`
to point the pet, `puck-agent`, or `puck-client` at a non-default socket
(useful for running more than one of each at once without them colliding).

### Making it your own

The avatar package format (`schema_version: 1`, `manifest.json` + clip PNGs)
is defined by puck-mac and read as-is here — an avatar folder built on
macOS should drop into Linux unchanged, same as it does on Windows today.
Field reference:
[puck-mac's README](https://github.com/desFernan/puck-mac#a-character).

## Community

Want to help plan the Linux port, or just curious about progress — join us on
**[Discord](https://discord.gg/ePBZVnwSYE)**.
