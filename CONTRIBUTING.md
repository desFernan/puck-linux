# Contributing

Thanks for looking. Puck is a desktop pet that is also an AI agent. This is
the Linux port, in Rust and GTK4 on X11.

## Where to start

- **A question, or an idea you want to talk through first** — the [Discussions tab](https://github.com/desFernan/puck-linux/discussions), or [Discord](https://discord.gg/nGqtBGP857).
  Discussions is searchable, Discord is faster; either is fine.
- **Something to work on** — issues labelled [`good first issue`](https://github.com/desFernan/puck-linux/issues?q=is%3Aopen+label%3A%22good+first+issue%22) are scoped small and do not
  assume you know the codebase.
- **A bug** — open an issue and say what you did, what happened, and what you
  expected. Your OS version and how you installed it help more than a stack
  trace on its own.

For anything larger than a fix, say so in an issue before you write it. Not as
a gate — so nobody writes the same thing twice, and so we can tell you early
if it is heading somewhere the design does not go.

## Build and test

```sh
cargo run --bin puck-linux -- /path/to/avatar-folder   # the pet
cargo run --bin puck-agent                             # terminal chat
cargo run --bin puck-client                            # GTK4 chat window

cargo test                 # everything
cargo test -p puck-core    # agent + bridge only
```

The pet and the chat window need GTK4 development headers (`libgtk-4-dev
libx11-dev` on Debian and Ubuntu, `gtk4-devel` plus the X11 devel packages on
Fedora) and an X11 session.

`puck-core` needs none of that. It is plain Rust with no desktop in it, so its
tests — wire format, the tool-call loop, real HTTP and socket round trips —
run anywhere, including on a machine with no GTK headers at all. You only need
the desktop toolchain to work on the pet and the window.

Bridge tests use a real Unix socket at a temp path. `PUCK_BRIDGE_SOCKET`
points a binary at a non-default socket, which is how you run more than one at
once without them colliding.

## Porting between platforms

This is one product with several implementations — [macOS](https://github.com/desFernan/puck-mac) and [Windows](https://github.com/desFernan/puck-windows) — and a Rust
rewrite besides. When commits land on one of them, a bot opens or updates a
`needs- port-from-<repo>` issue here listing what arrived. Those issues are
the backlog of what this port is missing, and they are the easiest place to
find work: every entry has a working implementation on another platform that
you can read before you write a line.

A port does not have to be a translation. The platform's own idiom wins over
matching the original file-for-file. What has to match is the behaviour a user
sees, and the tool names and wire formats the agent depends on.

## Commits and pull requests

Commit subjects here read as one imperative sentence saying what changes for
the person using it — "Let the pet come home to the island", not "refactor
island handling". Some carry a conventional-commit prefix (`chore:`, `fix:`)
and many do not; both are in the history. Match what you see there.

Keep a pull request to one change. If you find a second thing on the way, a
second PR is easier to review and easier to revert.

Run `cargo test` before you open it. CI runs them again, but finding it
yourself is faster than a round trip.

## Artwork and assets

Do not add artwork, icons, fonts or audio unless you made them or they are CC0
or public domain, and say which in the pull request. The code is MIT; the
assets beside it are not necessarily — see [LICENSE-ASSETS.md](LICENSE-ASSETS.md). A custom avatar you made for
yourself belongs in your own customisation folder, or in a Discussions post,
rather than in this repository.
