# Puck for Linux — Pet Overlay MVP Design

Status: Approved
Date: 2026-08-24

## Context

`puck-linux` is currently a placeholder repo. The macOS app
([desFernan/puck-mac](https://github.com/desFernan/puck-mac)) is a desktop
pet + AI agent built from two Swift apps (`Puck`, the always-on-top pet
overlay, and `PuckClient`, its chat/editor/terminal window) talking over a
local socket bridge, plus deep macOS integration (Accessibility API,
AppleScript) for the agent's device-control tools.

Porting the whole thing at once is too large for one spec: pet overlay,
agent core (chat/tools/approvals/sessions), client app (chat/editor/terminal),
the socket bridge, and Linux-native OS automation (an AppleScript/Accessibility
equivalent for X11/Wayland) are each independently sized projects. This spec
covers only the first slice: **the pet overlay app**, with no agent
functionality. Later specs will cover the agent core, the client app, and the
bridge between them.

## Goals (this spec)

- An always-on-top, transparent, undecorated window on Linux showing an
  animated character.
- Load an avatar from a folder containing `manifest.json` + PNG sprites,
  using the same schema puck-mac uses (`schema_version: 1`, `idle` is the
  only required clip, others fall back to it).
- Idle and walk animations; the character walks along the screen and turns
  around at screen edges.
- Mouse drag picks the character up; releasing drops it into a simple fall
  simulation that lands back on the ground.

## Non-goals (this spec)

- Any agent functionality: chat, LLM calls, tool execution, sessions,
  approvals.
- The `PuckClient`-equivalent window (editor, terminal, workspaces).
- The socket bridge between pet and client.
- Wayland layer-shell support — X11 (override-redirect always-on-top window)
  is the only supported display server for this slice. Wayland support is a
  follow-up spec once a compositor-portable "always on top, click-through
  where empty" approach is validated.
- Sound playback, emotion clips, `bounce_intensity` squash-and-stretch, or
  any manifest field beyond `schema_version`, `name`, `type`, `hitbox`,
  `clips`.
- A bundled default avatar. The app requires the user to point it at an
  avatar folder; there is no first-run experience.

## Tech stack

Rust + GTK4 (via `gtk4-rs`). Rationale: native Linux desktop toolkit,
mature X11 always-on-top/transparent window support, no extra runtime to
ship, good fit for a small always-running background app.

## Architecture

Single Rust binary, GTK4 application. A `DrawingArea` widget renders the
current sprite frame; a `glib::timeout_add_local` driven tick advances
animation frames and motion physics on a fixed interval (e.g. ~16ms /
60fps target, coalesced to whatever GTK's main loop actually delivers).

State machine driving which clip plays:

```
Idle -> Walk -> Idle   (walk episodes interspersed with idle, timer-driven)
Walk -> Fall           (walked off... no-op for MVP: turn around at edges instead)
(mouse press+drag) -> Drag -> (release) -> Fall -> Land -> Idle
```

`Drag` is not a manifest clip — while dragged the character shows `idle`
(or `fall` if present) following the cursor; on release it transitions into
`fall` until it reaches the ground, then `land`, then back to `idle`.

## Components

- **`avatar` module** — parses `manifest.json` into a struct, validates
  `schema_version == 1` and the presence of an `idle` clip, resolves clip
  names to sibling PNG paths, loads them as `gdk::Texture` /
  `gdk_pixbuf::Pixbuf`. Rejects manifests with paths that escape the
  package directory (matches puck-mac's stated behavior).
- **`window` module** — builds the `gtk::ApplicationWindow`: undecorated,
  transparent (via CSS `background: transparent` + a compositor-backed
  visual), always-on-top (`gtk::Window::set_keep_above`-equivalent via
  X11 window hints, since GTK4 dropped `set_keep_above` — use
  `gdk_x11` hints directly or `gtk_layer_shell` if available at build time,
  falling back to plain X11 hints otherwise).
- **`animation` module** — given the current clip name and elapsed time,
  returns which frame/texture to draw. MVP clips are single-frame (one PNG
  per clip, matching puck-mac's sprite model), so this module's job is
  picking the active clip's texture, not frame-sequencing within a clip.
- **`motion` module** — owns position, velocity, facing direction, and the
  state machine above. Computes walk movement and screen-edge turnarounds
  using the monitor geometry (`gdk::Monitor::geometry`). Implements a
  simple gravity fall (constant acceleration) for the drag-release case.
- **`main.rs`** — parses a CLI arg / env var for the avatar folder path,
  wires up the GTK `Application`, constructs the above, and starts the
  tick loop.

## Data flow

1. Startup: `main` reads the avatar path, `avatar::load` parses the
   manifest and textures, or the process exits with a clear error on the
   two rejection cases (missing `idle`, unparseable manifest / bad
   `schema_version`).
2. Each tick: `motion` advances position/state given elapsed time and any
   pending input event; `animation` maps the resulting clip name to a
   texture; `window`'s `DrawingArea` redraws with that texture at the
   window's current position (the window itself is moved to track
   position, since it's sized to the sprite).
3. Mouse input (press/motion/release on the `DrawingArea` or window) feeds
   into `motion` to drive the Drag/Fall transition.

## Error handling

- Manifest missing, unparseable, or missing `idle` clip, or with a path
  that escapes the package directory: print a clear error to stderr and
  exit non-zero. No fallback avatar in this slice.
- Unknown `schema_version`: same treatment.
- Missing PNG referenced by a clip: same treatment (fail at load time, not
  lazily during animation).

## Testing

- Unit tests for `avatar::load` (valid manifest, missing `idle`, bad
  `schema_version`, path traversal rejection) using fixture folders under
  `tests/fixtures/`.
- Unit tests for the `motion` state machine's transitions (edge turnaround,
  drag→fall→land→idle) driven by fake time steps, independent of GTK.
- Window/rendering behavior is verified manually (documented run steps in
  the README): launch against a sample avatar, confirm it appears
  always-on-top, walks, turns at edges, and can be dragged and dropped.
