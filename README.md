# Puck for Linux

> Language: **English** (here) · [한국어](README.ko.md)

> Puck currently exists on macOS — see
> [desFernan/puck-mac](https://github.com/desFernan/puck-mac) for the full
> app. This repo is the Linux port; it currently has the pet overlay only
> (no agent yet — see Status below).
>
> Platforms: [macOS](https://github.com/desFernan/puck-mac) · [Windows](https://github.com/desFernan/puck-windows) · **Linux** (here)

### 💬 [Join the Discord](https://discord.gg/ePBZVnwSYE)

Bugs, feature requests, build help, or just want to hang out — the
[support server](https://discord.gg/ePBZVnwSYE) is the fastest way to reach
us. Come say hi!

## Status

The pet overlay MVP is here: an always-on-top, transparent, animated
character you can drag around, using the same avatar folder format as
[puck-mac](https://github.com/desFernan/puck-mac). No agent features yet.
The agent core, the `PuckClient`-equivalent window, and the socket bridge
between them are follow-up work.

### Build and run

Requires Rust and GTK4 development headers (`libgtk-4-dev libx11-dev` on
Debian/Ubuntu, `gtk4-devel` + X11 devel packages on Fedora) on an X11
session — Wayland isn't supported yet.

```sh
cargo run -- /path/to/avatar-folder
```

An avatar folder needs a `manifest.json` and a PNG per clip — see
[puck-mac's README](https://github.com/desFernan/puck-mac#a-character) for
the manifest schema. This port reads `schema_version`, `name`, `type`,
`hitbox`, and `clips`; `idle` is the only required clip, and `walk`/`fall`/
`land` are used if present (falling back to `idle` otherwise).

### Test

```sh
cargo test --bin puck-linux
```

(Not `cargo test --lib` — this crate is bin-only, no library target.)

### Agent providers

Not built yet on this port. macOS talks to the Anthropic or OpenAI API
directly for chat, and runs a vendored ACP agent under `node` for the
`code_editor` tool; that layer is meant to port with minimal changes once
it's this port's turn.

### Making it your own

The avatar package format (`schema_version: 1`, `manifest.json` + clip PNGs)
is defined by puck-mac and read as-is here — an avatar folder built on
macOS should drop into Linux unchanged, same as it does on Windows today.
Field reference:
[puck-mac's README](https://github.com/desFernan/puck-mac#a-character).

## Community

Want to help plan the Linux port, or just curious about progress — join us on
**[Discord](https://discord.gg/ePBZVnwSYE)**.
