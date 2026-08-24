# Pet Overlay MVP Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust + GTK4 Linux app that shows an always-on-top, transparent, animated pet loaded from a puck-mac-compatible avatar folder, walking the screen and draggable.

**Architecture:** Single binary. `avatar` parses and validates the manifest/sprite folder into resolved paths (pure, headless-testable — no GTK/display needed). `window` owns the GTK application, the X11 always-on-top/transparent surface, and texture loading/drawing. `motion` owns the idle/walk/drag/fall state machine and position, driven by a GTK tick timeout, tested headlessly with fake time deltas. `animation` maps state to the clip name to draw.

**Tech Stack:** Rust, `gtk4` (gtk4-rs), `gdk4-x11` + `x11rb` for the X11 always-on-top hint, `serde`/`serde_json` for the manifest, `tempfile` (dev-dependency) for fixture-based tests.

**Spec:** `docs/superpowers/specs/2026-08-24-linux-pet-mvp-design.md`

## Global Constraints

- X11 only for this slice (no Wayland layer-shell) — per spec Non-goals.
- Only `schema_version`, `name`, `type`, `hitbox`, `clips` manifest fields are read; `sounds`/`emotions`/`bounce_intensity` are ignored.
- No bundled default avatar — the app requires an avatar path argument and exits with a clear error if the manifest is invalid, `idle` is missing, a referenced PNG is missing, or a clip path escapes the package directory.
- Sprites are single-frame per clip (one PNG per clip name) — no in-clip frame sequencing.

---

### Task 1: Project scaffold + always-on-top transparent window

**Files:**
- Create: `Cargo.toml`
- Create: `src/main.rs`
- Create: `src/window.rs`

**Interfaces:**
- Produces: `window::PuckWindow::new(app: &gtk4::Application) -> PuckWindow` — wraps a `gtk4::ApplicationWindow`, already undecorated/transparent/always-on-top and shown. `PuckWindow::gtk_window(&self) -> &gtk4::ApplicationWindow` for later tasks to attach content/events to.

- [ ] **Step 1: Write `Cargo.toml`**

```toml
[package]
name = "puck-linux"
version = "0.1.0"
edition = "2021"

[dependencies]
gtk4 = "0.9"
gdk4-x11 = "0.9"
x11rb = "0.13"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Write `src/window.rs`**

```rust
use gdk4_x11::X11Surface;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{AtomEnum, ConnectionExt};
use x11rb::rust_connection::RustConnection;

pub struct PuckWindow {
    window: ApplicationWindow,
}

impl PuckWindow {
    pub fn new(app: &Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .default_width(200)
            .default_height(200)
            .build();

        // Transparent background: the window's CSS background is set to
        // fully transparent; content drawn on top (added in Task 3) shows
        // through everywhere else.
        let css = gtk4::CssProvider::new();
        css.load_from_data("window { background-color: transparent; }");
        gtk4::style_context_add_provider_for_display(
            &gtk4::prelude::WidgetExt::display(&window),
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        window.present();
        set_always_on_top(&window);

        Self { window }
    }

    pub fn gtk_window(&self) -> &ApplicationWindow {
        &self.window
    }
}

/// Sets the EWMH `_NET_WM_STATE_ABOVE` hint on the window's underlying X11
/// surface. GTK4 dropped `Window::set_keep_above`, so this talks to the X
/// server directly. No-op (logs a warning) if the surface isn't X11 — e.g.
/// running under Wayland, which is out of scope for this slice.
fn set_always_on_top(window: &ApplicationWindow) {
    let Some(surface) = window.surface() else {
        eprintln!("puck: window has no surface yet, cannot set always-on-top");
        return;
    };
    let Ok(x11_surface) = surface.downcast::<X11Surface>() else {
        eprintln!("puck: not running on X11, always-on-top is unsupported in this build");
        return;
    };
    let xid = x11_surface.xid() as u32;

    let Ok((conn, screen_num)) = RustConnection::connect(None) else {
        eprintln!("puck: could not open X11 connection for always-on-top hint");
        return;
    };
    let root = conn.setup().roots[screen_num].root;

    let wm_state = intern_atom(&conn, "_NET_WM_STATE");
    let wm_state_above = intern_atom(&conn, "_NET_WM_STATE_ABOVE");
    let (Some(wm_state), Some(wm_state_above)) = (wm_state, wm_state_above) else {
        eprintln!("puck: could not resolve EWMH atoms for always-on-top hint");
        return;
    };

    let data = [
        1u32, // _NET_WM_STATE_ADD
        wm_state_above,
        0,
        0,
        0,
    ];
    let event = x11rb::protocol::xproto::ClientMessageEvent::new(32, xid, wm_state, data);
    let _ = conn.send_event(
        false,
        root,
        x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_REDIRECT
            | x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    );
    let _ = conn.flush();
}

fn intern_atom(conn: &RustConnection, name: &str) -> Option<u32> {
    conn.intern_atom(false, name.as_bytes())
        .ok()?
        .reply()
        .ok()
        .map(|r| r.atom)
}
```

- [ ] **Step 3: Write `src/main.rs`**

```rust
mod window;

use gtk4::prelude::*;
use gtk4::Application;

fn main() {
    let avatar_path = std::env::args().nth(1);
    let Some(avatar_path) = avatar_path else {
        eprintln!("usage: puck-linux <avatar-folder>");
        std::process::exit(1);
    };
    // Consumed by avatar::load in Task 2 — kept here for now so `cargo run`
    // has a clear usage error even before avatar loading exists.
    let _ = avatar_path;

    let app = Application::builder()
        .application_id("dev.desfernan.PuckLinux")
        .build();

    app.connect_activate(|app| {
        let _win = window::PuckWindow::new(app);
    });

    app.run();
}
```

- [ ] **Step 4: Build it**

Run: `cargo build`
Expected: compiles cleanly (fix any API drift against the installed `gtk4`/`gdk4-x11`/`x11rb` crate versions — pin exact versions in `Cargo.toml` to whatever `cargo build` resolves).

- [ ] **Step 5: Manual check**

Run: `cargo run -- /tmp/nonexistent-avatar`
Expected: an undecorated, transparent, always-on-top 200x200 window appears (visually confirm it stays above other windows when you click another window). It's fine that `/tmp/nonexistent-avatar` isn't validated yet — that's Task 2/3.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/main.rs src/window.rs
git commit -m "feat: scaffold always-on-top transparent GTK4 window"
```

---

### Task 2: Avatar manifest parsing and validation

**Files:**
- Create: `src/avatar.rs`
- Modify: `src/main.rs` (add `mod avatar;`)

**Interfaces:**
- Consumes: nothing from earlier tasks (pure module, no GTK).
- Produces:
  - `avatar::Avatar { pub hitbox: Hitbox, pub clips: HashMap<String, PathBuf> }` where `Hitbox { pub width: f64, pub height: f64 }`.
  - `avatar::load(dir: &Path) -> Result<Avatar, avatar::LoadError>`.
  - `avatar::LoadError` enum with `Display` impl covering: `Io`, `Parse(serde_json::Error)`, `UnsupportedSchemaVersion(u32)`, `MissingIdleClip`, `PathEscapesPackage(String)`, `MissingClipFile { clip: String, path: PathBuf }`. Later tasks (window/main) match on this to print the error and exit non-zero.

- [ ] **Step 1: Write the failing tests**

```rust
// bottom of src/avatar.rs
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_manifest(dir: &std::path::Path, json: &str) {
        fs::write(dir.join("manifest.json"), json).unwrap();
    }

    #[test]
    fn loads_minimal_valid_manifest() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 1,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 130, "height": 133 },
                "clips": { "idle": "idle" }
            }"#,
        );
        fs::write(dir.path().join("idle.png"), b"fake-png-bytes").unwrap();

        let avatar = load(dir.path()).unwrap();
        assert_eq!(avatar.hitbox.width, 130.0);
        assert_eq!(avatar.hitbox.height, 133.0);
        assert_eq!(avatar.clips.get("idle").unwrap(), &dir.path().join("idle.png"));
    }

    #[test]
    fn rejects_missing_idle_clip() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 1,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 10, "height": 10 },
                "clips": { "walk": "walk" }
            }"#,
        );
        fs::write(dir.path().join("walk.png"), b"fake").unwrap();

        assert!(matches!(load(dir.path()), Err(LoadError::MissingIdleClip)));
    }

    #[test]
    fn rejects_unsupported_schema_version() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 2,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 10, "height": 10 },
                "clips": { "idle": "idle" }
            }"#,
        );
        fs::write(dir.path().join("idle.png"), b"fake").unwrap();

        assert!(matches!(
            load(dir.path()),
            Err(LoadError::UnsupportedSchemaVersion(2))
        ));
    }

    #[test]
    fn rejects_path_traversal_in_clip_stem() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 1,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 10, "height": 10 },
                "clips": { "idle": "../escape" }
            }"#,
        );

        assert!(matches!(load(dir.path()), Err(LoadError::PathEscapesPackage(_))));
    }

    #[test]
    fn rejects_missing_clip_file() {
        let dir = tempdir().unwrap();
        write_manifest(
            dir.path(),
            r#"{
                "schema_version": 1,
                "name": "my-pet",
                "type": "sprites",
                "hitbox": { "width": 10, "height": 10 },
                "clips": { "idle": "idle" }
            }"#,
        );
        // idle.png intentionally not written

        assert!(matches!(
            load(dir.path()),
            Err(LoadError::MissingClipFile { .. })
        ));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail (module doesn't exist yet)**

Run: `cargo test avatar:: --lib`
Expected: FAIL to compile — `avatar` module / `load` / `LoadError` not defined.

- [ ] **Step 3: Write the implementation above the test module in `src/avatar.rs`**

```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
struct ManifestFile {
    schema_version: u32,
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    type_: String,
    hitbox: Hitbox,
    clips: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone, Copy)]
pub struct Hitbox {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug)]
pub struct Avatar {
    pub hitbox: Hitbox,
    pub clips: HashMap<String, PathBuf>,
}

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Parse(serde_json::Error),
    UnsupportedSchemaVersion(u32),
    MissingIdleClip,
    PathEscapesPackage(String),
    MissingClipFile { clip: String, path: PathBuf },
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::Io(e) => write!(f, "could not read avatar package: {e}"),
            LoadError::Parse(e) => write!(f, "manifest.json is not valid JSON: {e}"),
            LoadError::UnsupportedSchemaVersion(v) => {
                write!(f, "unsupported manifest schema_version: {v}")
            }
            LoadError::MissingIdleClip => {
                write!(f, "manifest.json must define an 'idle' clip")
            }
            LoadError::PathEscapesPackage(stem) => {
                write!(f, "clip path '{stem}' escapes the avatar package directory")
            }
            LoadError::MissingClipFile { clip, path } => {
                write!(f, "clip '{clip}' references missing file {}", path.display())
            }
        }
    }
}

impl std::error::Error for LoadError {}

pub fn load(dir: &Path) -> Result<Avatar, LoadError> {
    let manifest_path = dir.join("manifest.json");
    let raw = std::fs::read_to_string(&manifest_path).map_err(LoadError::Io)?;
    let manifest: ManifestFile = serde_json::from_str(&raw).map_err(LoadError::Parse)?;

    if manifest.schema_version != 1 {
        return Err(LoadError::UnsupportedSchemaVersion(manifest.schema_version));
    }
    if !manifest.clips.contains_key("idle") {
        return Err(LoadError::MissingIdleClip);
    }

    let mut clips = HashMap::new();
    for (clip_name, stem) in manifest.clips {
        if stem.contains("..") || Path::new(&stem).is_absolute() {
            return Err(LoadError::PathEscapesPackage(stem));
        }
        let file_path = dir.join(format!("{stem}.png"));
        if !file_path.exists() {
            return Err(LoadError::MissingClipFile {
                clip: clip_name,
                path: file_path,
            });
        }
        clips.insert(clip_name, file_path);
    }

    Ok(Avatar {
        hitbox: manifest.hitbox,
        clips,
    })
}
```

- [ ] **Step 4: Add `mod avatar;` to `src/main.rs`**

```rust
mod avatar;
mod window;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test avatar:: --lib`
Expected: all 5 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/avatar.rs src/main.rs
git commit -m "feat: parse and validate puck-mac-compatible avatar manifests"
```

---

### Task 3: Render the loaded avatar's idle sprite

**Files:**
- Modify: `src/main.rs`
- Modify: `src/window.rs`

**Interfaces:**
- Consumes: `avatar::load`, `avatar::Avatar`, `avatar::LoadError` (Task 2); `window::PuckWindow::new` (Task 1).
- Produces: `window::PuckWindow::set_texture(&self, path: &Path)` — loads a PNG from disk and draws it, resizing the window to the image's native size. Later tasks call this whenever the active clip changes.

- [ ] **Step 1: Extend `src/window.rs`: add a `DrawingArea` and `set_texture`**

Add near the top:

```rust
use gtk4::{DrawingArea};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
```

Change `PuckWindow` to hold the drawing area and current texture, and wire it into `new`:

```rust
pub struct PuckWindow {
    window: ApplicationWindow,
    drawing_area: DrawingArea,
    texture: Rc<RefCell<Option<gtk4::gdk::Texture>>>,
}

impl PuckWindow {
    pub fn new(app: &Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .default_width(200)
            .default_height(200)
            .build();

        let css = gtk4::CssProvider::new();
        css.load_from_data("window { background-color: transparent; }");
        gtk4::style_context_add_provider_for_display(
            &gtk4::prelude::WidgetExt::display(&window),
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        let drawing_area = DrawingArea::new();
        window.set_child(Some(&drawing_area));

        let texture: Rc<RefCell<Option<gtk4::gdk::Texture>>> = Rc::new(RefCell::new(None));
        let texture_for_draw = texture.clone();
        drawing_area.set_draw_func(move |_area, ctx, width, height| {
            if let Some(tex) = texture_for_draw.borrow().as_ref() {
                ctx.set_source_pixbuf(
                    &gtk4::gdk_pixbuf::Pixbuf::from(tex.clone()),
                    0.0,
                    0.0,
                );
                let _ = ctx.paint();
            }
            let _ = (width, height);
        });

        window.present();
        set_always_on_top(&window);

        Self {
            window,
            drawing_area,
            texture,
        }
    }

    pub fn gtk_window(&self) -> &ApplicationWindow {
        &self.window
    }

    /// Loads the PNG at `path`, resizes the window to its native size, and
    /// redraws. Panics if the file can't be decoded — callers are expected
    /// to have already validated the path exists via `avatar::load`.
    pub fn set_texture(&self, path: &Path) {
        let tex = gtk4::gdk::Texture::from_filename(path)
            .unwrap_or_else(|e| panic!("failed to decode {}: {e}", path.display()));
        self.window.set_default_size(tex.width(), tex.height());
        self.drawing_area.set_content_width(tex.width());
        self.drawing_area.set_content_height(tex.height());
        *self.texture.borrow_mut() = Some(tex);
        self.drawing_area.queue_draw();
    }
}
```

`gdk_pixbuf::Pixbuf::from(tex.clone())` is a placeholder for whatever the
installed `gtk4` version's actual texture-to-drawable path is (e.g.
`Texture::download` into a `cairo::ImageSurface`, or `ctx.set_source_surface`
via `tex.surface()` if available) — resolve against the exact API surface
`cargo doc` shows for the resolved `gtk4` version and adjust this snippet to
compile; the draw func's job (fill the `DrawingArea` with the current
texture) does not change.

- [ ] **Step 2: Wire it up in `src/main.rs`**

```rust
mod avatar;
mod window;

use gtk4::prelude::*;
use gtk4::Application;

fn main() {
    let avatar_path = std::env::args().nth(1);
    let Some(avatar_path) = avatar_path else {
        eprintln!("usage: puck-linux <avatar-folder>");
        std::process::exit(1);
    };
    let avatar_path = std::path::PathBuf::from(avatar_path);

    let loaded = match avatar::load(&avatar_path) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("puck: {e}");
            std::process::exit(1);
        }
    };

    let app = Application::builder()
        .application_id("dev.desfernan.PuckLinux")
        .build();

    app.connect_activate(move |app| {
        let win = window::PuckWindow::new(app);
        let idle_path = loaded.clips.get("idle").expect("idle validated by avatar::load");
        win.set_texture(idle_path);
        // Leak the window so it isn't dropped when `connect_activate` returns;
        // Task 4 replaces this with proper ownership via the motion loop.
        std::mem::forget(win);
    });

    app.run();
}
```

- [ ] **Step 3: Build and manually verify**

Run: `cargo build`
Expected: compiles (adjust the texture-drawing snippet per Step 1's note if the exact `gtk4` version's API differs).

Run: `cargo run -- tests/fixtures/avatars/valid` (create this fixture: a `manifest.json` matching Task 2's minimal example plus a real small PNG named `idle.png` — any tiny valid PNG works, e.g. exported from an image editor or `convert -size 32x32 xc:blue idle.png` with ImageMagick)
Expected: the window appears showing the idle sprite instead of being blank.

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/window.rs tests/fixtures/avatars/valid
git commit -m "feat: render the avatar's idle sprite in the overlay window"
```

---

### Task 4: Idle/walk state machine with screen-edge turnaround

**Files:**
- Create: `src/motion.rs`
- Modify: `src/main.rs` (add `mod motion;`, wire the tick loop)
- Modify: `src/window.rs` (add `move_to(&self, x: i32, y: i32)`)

**Interfaces:**
- Consumes: `avatar::Avatar.hitbox` (Task 2) for the sprite's width when computing edge turnaround; `window::PuckWindow::set_texture`, `window::PuckWindow::move_to` (this task adds `move_to`).
- Produces:
  - `motion::State { Idle, Walk }` (`Drag`/`Fall`/`Land` added in Task 5).
  - `motion::Motion::new(hitbox_width: f64, screen_width: f64) -> Motion`.
  - `motion::Motion::tick(&mut self, dt_secs: f64) -> motion::Frame` where `Frame { pub clip: &'static str, pub x: f64, pub facing_right: bool }`. Called every tick by `main.rs`'s timeout; `x` is the sprite's left edge in screen coordinates.

- [ ] **Step 1: Write the failing tests**

```rust
// bottom of src/motion.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_idle_at_x_zero() {
        let mut m = Motion::new(50.0, 800.0);
        let f = m.tick(0.0);
        assert_eq!(f.clip, "idle");
        assert_eq!(f.x, 0.0);
    }

    #[test]
    fn walking_moves_right_and_faces_right() {
        let mut m = Motion::new(50.0, 800.0);
        m.force_state_for_test(State::Walk, true);
        let f = m.tick(1.0);
        assert_eq!(f.clip, "walk");
        assert!(f.x > 0.0);
        assert!(f.facing_right);
    }

    #[test]
    fn turns_around_at_right_edge() {
        let mut m = Motion::new(50.0, 100.0);
        m.force_state_for_test(State::Walk, true);
        // screen_width 100, hitbox 50 -> right edge for sprite's left-x is 50.
        // Walk far enough right to hit it.
        for _ in 0..1000 {
            m.tick(0.1);
        }
        let f = m.tick(0.0);
        assert!(!f.facing_right, "should have turned around before falling off the right edge");
        assert!(f.x <= 50.0);
        assert!(f.x >= 0.0);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test motion:: --lib`
Expected: FAIL to compile — `motion` module doesn't exist.

- [ ] **Step 3: Implement `src/motion.rs`**

```rust
const WALK_SPEED_PX_PER_SEC: f64 = 40.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Walk,
}

pub struct Frame {
    pub clip: &'static str,
    pub x: f64,
    pub facing_right: bool,
}

pub struct Motion {
    state: State,
    x: f64,
    facing_right: bool,
    hitbox_width: f64,
    screen_width: f64,
    time_in_state: f64,
    idle_duration: f64,
    walk_duration: f64,
}

impl Motion {
    pub fn new(hitbox_width: f64, screen_width: f64) -> Self {
        Self {
            state: State::Idle,
            x: 0.0,
            facing_right: true,
            hitbox_width,
            screen_width,
            time_in_state: 0.0,
            idle_duration: 3.0,
            walk_duration: 4.0,
        }
    }

    #[cfg(test)]
    pub fn force_state_for_test(&mut self, state: State, facing_right: bool) {
        self.state = state;
        self.facing_right = facing_right;
        self.time_in_state = 0.0;
    }

    pub fn tick(&mut self, dt_secs: f64) -> Frame {
        self.time_in_state += dt_secs;

        match self.state {
            State::Idle => {
                if self.time_in_state >= self.idle_duration {
                    self.state = State::Walk;
                    self.time_in_state = 0.0;
                }
            }
            State::Walk => {
                let delta = WALK_SPEED_PX_PER_SEC * dt_secs;
                let max_x = (self.screen_width - self.hitbox_width).max(0.0);

                if self.facing_right {
                    self.x += delta;
                    if self.x >= max_x {
                        self.x = max_x;
                        self.facing_right = false;
                    }
                } else {
                    self.x -= delta;
                    if self.x <= 0.0 {
                        self.x = 0.0;
                        self.facing_right = true;
                    }
                }

                if self.time_in_state >= self.walk_duration {
                    self.state = State::Idle;
                    self.time_in_state = 0.0;
                }
            }
        }

        Frame {
            clip: match self.state {
                State::Idle => "idle",
                State::Walk => "walk",
            },
            x: self.x,
            facing_right: self.facing_right,
        }
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test motion:: --lib`
Expected: all 3 tests PASS.

- [ ] **Step 5: Add `move_to` to `src/window.rs`**

```rust
pub fn move_to(&self, x: i32, _y: i32) {
    // GTK4 top-level windows can't be positioned directly on Wayland, and
    // on X11 positioning goes through the surface. For this X11-only slice,
    // use the window's native X11 surface to move it.
    if let Some(surface) = self.window.surface() {
        if let Ok(x11_surface) = surface.downcast::<gdk4_x11::X11Surface>() {
            // gdk4-x11 does not expose a direct move; this is done via the
            // same x11rb connection pattern as `set_always_on_top` in
            // window.rs, issuing a ConfigureWindow request for `x11_surface.xid()`.
            // Left for the implementer to wire using x11rb::protocol::xproto::configure_window
            // with x = x, y = current_y — mirror the connection setup already
            // written in `set_always_on_top`.
            let _ = x11_surface;
        }
    }
    let _ = x;
}
```

This step's body is intentionally a stub *comment describing the exact call
to make* (`configure_window` with the existing `set_always_on_top` X11
connection pattern) rather than no-op silence — wire it for real using the
same `RustConnection::connect` + `ConnectionExt` pattern from Task 1's
`set_always_on_top`, calling `conn.configure_window(xid, &ConfigureWindowAux::new().x(x).y(y))`.
Do this now, before Step 6, so movement is actually visible.

- [ ] **Step 6: Wire the tick loop in `src/main.rs`**

Add `mod motion;`, and inside `connect_activate`, replace the `std::mem::forget(win)` block:

```rust
app.connect_activate(move |app| {
    let win = std::rc::Rc::new(window::PuckWindow::new(app));
    let idle_path = loaded.clips.get("idle").expect("idle validated by avatar::load").clone();
    let walk_path = loaded.clips.get("walk").cloned();
    win.set_texture(&idle_path);

    let screen_width = win
        .gtk_window()
        .display()
        .monitors()
        .item(0)
        .and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok())
        .map(|m| m.geometry().width() as f64)
        .unwrap_or(1920.0);

    let motion = std::rc::Rc::new(std::cell::RefCell::new(
        motion::Motion::new(loaded.hitbox.width, screen_width),
    ));
    let last_clip = std::rc::Rc::new(std::cell::RefCell::new("idle"));

    let win_for_tick = win.clone();
    let motion_for_tick = motion.clone();
    gtk4::glib::source::timeout_add_local(std::time::Duration::from_millis(16), move || {
        let frame = motion_for_tick.borrow_mut().tick(0.016);
        if frame.clip != *last_clip.borrow() {
            let path = if frame.clip == "walk" {
                walk_path.as_ref().unwrap_or(&idle_path)
            } else {
                &idle_path
            };
            win_for_tick.set_texture(path);
            *last_clip.borrow_mut() = frame.clip;
        }
        win_for_tick.move_to(frame.x as i32, 0);
        gtk4::glib::ControlFlow::Continue
    });

    std::mem::forget(win);
});
```

- [ ] **Step 7: Build and manually verify**

Run: `cargo build`
Expected: compiles.

Run: `cargo run -- tests/fixtures/avatars/valid` with a `walk.png` added to that fixture (matching manifest `"clips": {"idle": "idle", "walk": "walk"}`).
Expected: after ~3s the sprite starts moving right, swaps to the walk image, and turns around before reaching the screen edge.

- [ ] **Step 8: Commit**

```bash
git add src/motion.rs src/main.rs src/window.rs
git commit -m "feat: idle/walk state machine with screen-edge turnaround"
```

---

### Task 5: Drag and drop with fall/land physics

**Files:**
- Modify: `src/motion.rs`
- Modify: `src/main.rs`
- Modify: `src/window.rs` (expose mouse event hookup)

**Interfaces:**
- Consumes: `motion::Motion`, `motion::State`, `motion::Frame` (Task 4); `window::PuckWindow` (Tasks 1/3/4).
- Produces:
  - Extended `motion::State { Idle, Walk, Drag, Fall, Land }`.
  - `motion::Motion::begin_drag(&mut self, x: f64, y: f64)`, `motion::Motion::drag_to(&mut self, x: f64, y: f64)`, `motion::Motion::end_drag(&mut self)` — release starts a fall.
  - `Frame` gains `pub y: f64` (ground is `y = 0`, gravity increases `y` downward... conventionally screen-down; tests treat `y` as "distance fallen so far since drag release", `0.0` while grounded).

- [ ] **Step 1: Write the failing tests (append to `src/motion.rs`'s test module)**

```rust
#[test]
fn drag_moves_freely_and_release_starts_fall() {
    let mut m = Motion::new(50.0, 800.0);
    m.begin_drag(10.0, 5.0);
    let f = m.tick(0.1);
    assert_eq!(f.clip, "idle"); // dragged sprite shows idle
    assert_eq!(f.x, 10.0);
    assert_eq!(f.y, 5.0);

    m.drag_to(20.0, 5.0);
    let f = m.tick(0.1);
    assert_eq!(f.x, 20.0);

    m.end_drag();
    let f = m.tick(0.1);
    assert_eq!(f.clip, "fall");
    assert!(f.y > 5.0, "should have started falling (gravity increases y)");
}

#[test]
fn fall_lands_at_ground_and_returns_to_idle() {
    let mut m = Motion::new(50.0, 800.0);
    m.begin_drag(0.0, 0.0);
    m.end_drag();
    // Simulate a long fall; ground is y = GROUND_Y.
    for _ in 0..1000 {
        let f = m.tick(0.05);
        if f.clip == "idle" {
            return; // reached idle again after landing - test passes
        }
    }
    panic!("never returned to idle after falling");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test motion:: --lib`
Expected: FAIL to compile — `begin_drag`/`drag_to`/`end_drag`/`Frame.y` don't exist yet.

- [ ] **Step 3: Extend `src/motion.rs`**

Add to the top-level consts:

```rust
const GRAVITY_PX_PER_SEC2: f64 = 800.0;
const GROUND_Y: f64 = 0.0;
const LAND_DURATION_SECS: f64 = 0.3;
```

Update `State`, `Frame`, and `Motion`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Idle,
    Walk,
    Drag,
    Fall,
    Land,
}

pub struct Frame {
    pub clip: &'static str,
    pub x: f64,
    pub y: f64,
    pub facing_right: bool,
}
```

Add `y` and `fall_velocity` fields to `Motion`, initialized to `0.0` in `new`, and update every `Frame { .. }` construction in `tick` to include `y: self.y`. Add these methods:

```rust
pub fn begin_drag(&mut self, x: f64, y: f64) {
    self.state = State::Drag;
    self.x = x;
    self.y = y;
    self.fall_velocity = 0.0;
    self.time_in_state = 0.0;
}

pub fn drag_to(&mut self, x: f64, y: f64) {
    self.x = x;
    self.y = y;
}

pub fn end_drag(&mut self) {
    self.state = State::Fall;
    self.time_in_state = 0.0;
}
```

And extend `tick`'s match with:

```rust
State::Drag => {
    // Position is driven by drag_to(); nothing to advance here.
}
State::Fall => {
    self.fall_velocity += GRAVITY_PX_PER_SEC2 * dt_secs;
    self.y += self.fall_velocity * dt_secs;
    if self.y >= GROUND_Y {
        self.y = GROUND_Y;
        self.state = State::Land;
        self.time_in_state = 0.0;
    }
}
State::Land => {
    if self.time_in_state >= LAND_DURATION_SECS {
        self.state = State::Idle;
        self.time_in_state = 0.0;
    }
}
```

Note `y` starts at `0.0` (dropped from the current cursor height, which for
this MVP is always at/above `GROUND_Y = 0.0`) — since `begin_drag` sets `y`
to whatever the cursor's `y` was, and the ground is `0.0`, a drag that
starts below `0.0` needs inverting; for this MVP, treat `GROUND_Y` as the
*maximum* `y` (screen-down coordinates, ground is the largest `y`), so
`begin_drag(x, y)` where `y` is already close to `0` in the tests reflects
starting near the ground and falling *down* means `y` decreasing toward
more negative... **resolve the sign convention while making the two tests
above pass** — the test names encode the required behavior (`y` increases
after `end_drag`, reaching `GROUND_Y` ends the fall), so pick whichever sign
convention satisfies both; document the chosen convention in a comment
above `GROUND_Y`.

Update the `match self.state { ... }` clip-name block to include:

```rust
State::Drag => "idle",
State::Fall => "fall",
State::Land => "land",
```

(Fall back to `"idle"`/`"walk"` textures at the call site in `main.rs` if
the avatar has no `fall`/`land` clip — same pattern already used for
`walk_path.as_ref().unwrap_or(&idle_path)` in Task 4.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test motion:: --lib`
Expected: all 5 tests PASS (3 from Task 4 + 2 new).

- [ ] **Step 5: Wire mouse input in `src/window.rs`**

Add a method to attach a `gtk4::GestureDrag` to the drawing area, taking closures so `main.rs` decides what happens (keeps `window.rs` free of `motion` knowledge):

```rust
pub fn connect_drag<FBegin, FUpdate, FEnd>(&self, on_begin: FBegin, on_update: FUpdate, on_end: FEnd)
where
    FBegin: Fn(f64, f64) + 'static,
    FUpdate: Fn(f64, f64) + 'static,
    FEnd: Fn() + 'static,
{
    let gesture = gtk4::GestureDrag::new();
    gesture.connect_drag_begin(move |g, x, y| {
        let _ = g;
        on_begin(x, y);
    });
    gesture.connect_drag_update(move |g, x, y| {
        let (start_x, start_y) = g.start_point().unwrap_or((0.0, 0.0));
        on_update(start_x + x, start_y + y);
    });
    gesture.connect_drag_end(move |_g, _x, _y| {
        on_end();
    });
    self.drawing_area.add_controller(gesture);
}
```

- [ ] **Step 6: Wire it up in `src/main.rs`'s `connect_activate`**

After the existing tick-loop setup, before `std::mem::forget(win)`:

```rust
let motion_for_drag_begin = motion.clone();
let motion_for_drag_update = motion.clone();
let motion_for_drag_end = motion.clone();
win.connect_drag(
    move |x, y| motion_for_drag_begin.borrow_mut().begin_drag(x, y),
    move |x, y| motion_for_drag_update.borrow_mut().drag_to(x, y),
    move || motion_for_drag_end.borrow_mut().end_drag(),
);
```

Also extend the tick closure's clip-swap `if` to cover `"fall"`/`"land"`
by looking them up from `loaded.clips` the same way `walk_path` is looked
up, falling back to `idle_path` when the avatar doesn't define them.

- [ ] **Step 7: Build and manually verify**

Run: `cargo build`
Expected: compiles.

Run: `cargo run -- tests/fixtures/avatars/valid`
Expected: clicking and dragging the sprite moves it with the cursor; releasing drops it, and it settles back to idle at the ground.

- [ ] **Step 8: Commit**

```bash
git add src/motion.rs src/main.rs src/window.rs
git commit -m "feat: drag-and-drop with fall/land physics"
```

---

### Task 6: README run instructions

**Files:**
- Modify: `README.md`
- Modify: `README.ko.md`

**Interfaces:**
- Consumes: nothing (documentation only).

- [ ] **Step 1: Replace the "Status" section of `README.md`**

Replace the existing placeholder "Status" section with:

```markdown
## Status

The pet overlay MVP is here: an always-on-top, transparent, animated
character you can drag around, using the same avatar folder format as
[puck-mac](https://github.com/desFernan/puck-mac). No agent features yet —
see [`docs/superpowers/specs/2026-08-24-linux-pet-mvp-design.md`](docs/superpowers/specs/2026-08-24-linux-pet-mvp-design.md)
for what's in and out of scope.

### Build and run

Requires Rust and GTK4 development headers (`libgtk-4-dev` on Debian/Ubuntu,
`gtk4-devel` on Fedora) on an X11 session (Wayland isn't supported yet).

```sh
cargo run -- /path/to/avatar-folder
```

An avatar folder needs a `manifest.json` and a PNG per clip — see
puck-mac's README for the manifest schema (this port reads `schema_version`,
`name`, `type`, `hitbox`, and `clips`; `idle` is the only required clip).

### Test

```sh
cargo test
```
```

- [ ] **Step 2: Apply the same update to `README.ko.md`** (translate the section, keep the same structure and links)

- [ ] **Step 3: Commit**

```bash
git add README.md README.ko.md
git commit -m "docs: document pet overlay MVP build/run/test instructions"
```

---

## Self-Review Notes

- **Spec coverage:** always-on-top/transparent window (Task 1), avatar manifest loading incl. all four rejection cases and path-traversal (Task 2), idle sprite rendering (Task 3), walk + edge turnaround (Task 4), drag/fall/land (Task 5), manual run docs (Task 6), unit tests for avatar and motion modules headlessly (Tasks 2/4/5), manual verification steps for windowing (every task touching `window.rs`) — all spec sections have a task.
- **Known open API risk:** GTK4's exact texture→cairo drawing call and window positioning on X11 both depend on the precise `gtk4`/`gdk4-x11` crate versions `cargo build` resolves; Tasks 3 and 4 flag the exact spot to adjust and what the adjustment must accomplish, rather than leaving it vague — this is a real external-API uncertainty, not a deferred design decision.
