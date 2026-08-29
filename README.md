# Puck for Linux

> Language: **English** (here) · [한국어](README.ko.md)

> A Linux port of [**desFernan/puck-mac**](https://github.com/desFernan/puck-mac)
> (Swift/AppKit, macOS). Rust + GTK4, X11.
>
> Platforms: [macOS](https://github.com/desFernan/puck-mac) · [Windows](https://github.com/desFernan/puck-windows) · **Linux** (here)

### 💬 [Join the Discord](https://discord.gg/ePBZVnwSYE)

Bugs, feature requests, build help, or just want to hang out — the
[support server](https://discord.gg/ePBZVnwSYE) is the fastest way to reach
us. Come say hi!

A Linux desktop pet that is also an AI agent. Three Rust binaries:

- **`puck-linux`** — the pet: an always-on-top, transparent, animated character
  you can drag around, reading the same avatar folders as puck-mac.
- **`puck-agent`** — the agent in a terminal: a REPL against the Anthropic API,
  with `run_shell` gated behind per-call approval.
- **`puck-client`** — the same agent in a minimal GTK4 chat window, the current
  `PuckClient`-equivalent; approvals are a Yes/No dialog instead of a prompt.

The three talk over a local socket bridge (`src/bridge.rs`): while a front end
is working on a request it tells the pet to show a `thinking` clip, then
`happy` or `sad` depending on how the turn ended (falling back to `idle` if the
avatar doesn't define one). That is the first slice of puck-mac's
pet-talks-to-client architecture — one message so far, nothing richer yet: no
shared sessions, no forwarding chat into the pet. The agent core lives in
`src/agent/`.

Not ported yet: Wayland (X11 only, by design for this slice), and the code
editor, terminal pane and workspaces that puck-mac's real `PuckClient` has.

## Build

Needs Rust and GTK4 development headers (`libgtk-4-dev libx11-dev` on
Debian/Ubuntu, `gtk4-devel` plus the X11 devel packages on Fedora), on an X11
session.

```sh
cargo run --bin puck-linux -- /path/to/avatar-folder   # the pet
cargo run --bin puck-agent                             # terminal chat
cargo run --bin puck-client                            # GTK4 chat window
```

The pet takes the avatar folder as its one argument — see
[Making it your own](#making-it-your-own).

## Test

```sh
cargo test --bin puck-linux   # pet: parsing, animation/physics state machine, emotion override
cargo test --lib              # agent + bridge: wire format, tool-call loop, real HTTP/socket round trips
```

They are separate targets, so `--lib` and `--bin puck-linux` cannot share one
invocation; plain `cargo test` runs both, plus the `puck-agent`/`puck-client`
binaries' own (currently empty) test targets.

Bridge tests use a real Unix socket at a temp path. `PUCK_BRIDGE_SOCKET` points
the pet, `puck-agent` or `puck-client` at a non-default socket — useful for
running more than one of each at once without them colliding.

## Agent providers

Anthropic, called directly over HTTP. Both front ends read `ANTHROPIC_API_KEY`
from the environment, or from a `.env` file (`KEY=VALUE` per line) in the
current directory if the variable is not already set — matching puck-mac's
credential file. Both default to `claude-opus-5`; override with
`PUCK_AGENT_MODEL`.

The only tool so far is `run_shell`, which runs a command with the same
permissions as the agent process — it is **not** sandboxed or allowlisted.
Every call asks first, showing the tool name and the exact input: a `y`/`yes`
prompt in `puck-agent`, a Yes/No dialog in `puck-client`.

## Making it your own

An avatar is a folder with a `manifest.json` and one PNG per clip beside it,
and the pet is pointed at one directly:

```
my-pet/
    manifest.json
    idle.png  walk.png  fall.png  …
```

```sh
cargo run --bin puck-linux -- ./my-pet
```

### A character

One drawing is a working character — `idle` is the only clip that has to
exist, and this port also uses `walk`, `fall` and `land` when they are there,
falling back to `idle` otherwise. Transparent background, drawn facing right.
The smallest manifest that works:

```json
{
  "schema_version": 1,
  "name": "my-pet",
  "type": "sprites",
  "hitbox": { "width": 130, "height": 133 },
  "clips": { "idle": "idle" }
}
```

`hitbox` is the size it will be drawn at — match your drawing's proportions or
it will look squashed. `emotions` are read too, and are what the bridge swaps
in while the agent works.

If the package is wrong the pet does not start and says why on stderr — a
missing `idle` file, a manifest that will not parse, or a path that climbs out
of the package.

The package format (`schema_version: 1`) is defined by puck-mac and read here
as-is, so an avatar folder built on macOS drops in unchanged. The full field
reference — `clips`, `emotions`, `sounds`, `hitbox`, `bounce_intensity` and
what each one defaults to — lives in
[puck-mac's README](https://github.com/desFernan/puck-mac#a-character); this
port reads the subset above and ignores the rest.

## Community

Questions, bug reports, feature ideas, or just want to show off your custom
avatar — join us on **[Discord](https://discord.gg/ePBZVnwSYE)**.
