# Puck for Linux

> Language: **English** (here) · [한국어](README.ko.md)

> Not started yet. Puck currently only exists on macOS — see
> [desFernan/puck-mac](https://github.com/desFernan/puck-mac) for the real
> thing. This repo is a placeholder for a future Linux port; everything below
> is the intended shape, not built code.
>
> Platforms: [macOS](https://github.com/desFernan/puck-mac) · [Windows](https://github.com/desFernan/puck-windows) · **Linux** (here)

### 💬 [Join the Discord](https://discord.gg/ePBZVnwSYE)

Bugs, feature requests, build help, or just want to hang out — the
[support server](https://discord.gg/ePBZVnwSYE) is the fastest way to reach
us. Come say hi!

## Status

Nothing here yet — no code, no plan doc. [puck-windows](https://github.com/desFernan/puck-windows)
went C# / .NET 8 + WPF with a `docs/porting-design.md` written before any
code landed; the Linux port would start the same way: pick a stack, map
macOS's modules to it, write the phase plan, then port.

## What a Linux port would look like

Mirroring [puck-mac](https://github.com/desFernan/puck-mac) and
[puck-windows](https://github.com/desFernan/puck-windows) — same shape, once
there's code to back it:

### Build

A `pet-app/scripts/` build script, same as the other two ports. Stack (GTK?
Qt? something else entirely) is not decided.

### Test

An unattended test script that exits nonzero on failure, same contract as
`pet-app/scripts/test.sh` (macOS) and `pet-app/scripts/test.ps1` (Windows).

### Agent providers

Same design as macOS: normal chat talks to the Anthropic or OpenAI API
directly, and the `code_editor` tool runs a vendored ACP agent under `node`.
Not Linux-specific — this layer is meant to port with minimal changes.

### Making it your own

The avatar package format (`schema_version: 1`, `manifest.json` + clip PNGs)
is defined by puck-mac and meant to be read as-is on every port — an avatar
folder built on macOS should drop into Linux unchanged, same as it does on
Windows today. Field reference:
[puck-mac's README](https://github.com/desFernan/puck-mac#a-character).

## Community

Want to help plan the Linux port, or just curious about progress — join us on
**[Discord](https://discord.gg/ePBZVnwSYE)**.
