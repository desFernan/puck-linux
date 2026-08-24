use gdk4_x11::X11Surface;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea};
use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::rust_connection::RustConnection;

/// An X11 connection opened once and reused for every always-on-top hint
/// and every window move, instead of reconnecting per call — `move_to` runs
/// on every animation tick (60fps), so a fresh connection per call would
/// mean 60 socket connect/disconnect cycles a second.
struct X11State {
    conn: RustConnection,
    xid: u32,
    root: u32,
}

pub struct PuckWindow {
    window: ApplicationWindow,
    drawing_area: DrawingArea,
    texture: Rc<RefCell<Option<gtk4::gdk::Texture>>>,
    facing_right: Rc<Cell<bool>>,
    x11: Rc<RefCell<Option<X11State>>>,
    last_pos: Cell<Option<(i32, i32)>>,
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
        // fully transparent; content drawn on top shows through everywhere
        // else.
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
        let facing_right: Rc<Cell<bool>> = Rc::new(Cell::new(true));
        let texture_for_draw = texture.clone();
        let facing_right_for_draw = facing_right.clone();
        drawing_area.set_draw_func(move |_area, ctx, _width, _height| {
            let Some(tex) = texture_for_draw.borrow().clone() else {
                return;
            };
            // gdk4-rs 0.9 has no `From<Texture> for Pixbuf` (nor any other
            // direct texture -> drawable conversion): the only manual method
            // on `Texture` is `download()`, which writes raw pixel bytes in
            // Cairo's native `CAIRO_FORMAT_ARGB32` layout (premultiplied
            // alpha, host-endian) straight into a caller-provided buffer.
            // That layout is exactly what a `cairo::ImageSurface` in
            // `Format::ARgb32` expects, so we download directly into one and
            // paint it.
            let width = tex.width();
            let height = tex.height();
            let mut surface = match gtk4::cairo::ImageSurface::create(
                gtk4::cairo::Format::ARgb32,
                width,
                height,
            ) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("puck: failed to create image surface for texture: {e}");
                    return;
                }
            };
            let stride = surface.stride();
            {
                let mut data = match surface.data() {
                    Ok(d) => d,
                    Err(e) => {
                        eprintln!("puck: failed to borrow image surface data: {e}");
                        return;
                    }
                };
                tex.download(&mut data, stride as usize);
            }
            let _ = surface.flush();

            // The sprite is drawn facing right; walking left mirrors it
            // horizontally around its own center, matching puck-mac's
            // sprite convention.
            let flip = !facing_right_for_draw.get();
            if flip {
                let _ = ctx.save();
                ctx.translate(width as f64, 0.0);
                ctx.scale(-1.0, 1.0);
            }
            let painted = ctx.set_source_surface(&surface, 0.0, 0.0).and_then(|()| ctx.paint());
            if flip {
                let _ = ctx.restore();
            }
            if let Err(e) = painted {
                eprintln!("puck: failed to paint texture: {e}");
            }
        });

        // Send the always-on-top hint once the window is actually mapped,
        // rather than guessing a fixed delay. Sending the EWMH
        // _NET_WM_STATE ClientMessage synchronously right after `present()`
        // races the window manager: `present()` only queues the X
        // MapWindow request, and the surface isn't mapped yet at that
        // point, so the WM silently drops the message (confirmed
        // empirically: openbox ignores it when sent before the surface is
        // mapped, but honors the identical message once it is).
        //
        // `Widget::connect_map` fires exactly when GTK's underlying
        // GdkSurface has been mapped, which is the event that was actually
        // missing before — not a fixed amount of wall-clock time. Verified
        // empirically (gtk4 0.9.7, Xvfb + openbox): sending the
        // ClientMessage from `map` with *zero* extra delay reliably
        // results in `_NET_WM_STATE_ABOVE` being set, across repeated runs
        // and under artificial CPU load meant to simulate a slower system.
        //
        // Two alternatives were tried and ruled out before landing on this:
        // - `connect_realize`: fires even earlier than `map` (the surface
        //   exists but isn't mapped yet) — same race as sending
        //   synchronously after `present()`, still effectively a guess.
        // - Setting `_NET_WM_STATE` as a plain property (`XChangeProperty`)
        //   before mapping, which EWMH nominally allows for withdrawn
        //   windows: empirically, GTK4 overwrites/clobbers this property
        //   with its own state list at some point during its own map
        //   processing, silently wiping the hint before the WM honors it.
        //
        // The 200ms timeout is kept as a fallback safety net in case `map`
        // doesn't fire as expected on some other WM/compositor combination
        // — re-sending the ClientMessage is idempotent, so both firing is
        // harmless.
        let x11: Rc<RefCell<Option<X11State>>> = Rc::new(RefCell::new(None));

        let window_for_map = window.clone();
        let x11_for_map = x11.clone();
        window.connect_map(move |_w| {
            set_always_on_top(&window_for_map, &x11_for_map);
        });

        window.present();

        let window_for_timeout = window.clone();
        let x11_for_timeout = x11.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            set_always_on_top(&window_for_timeout, &x11_for_timeout);
        });

        Self {
            window,
            drawing_area,
            texture,
            facing_right,
            x11,
            last_pos: Cell::new(None),
        }
    }

    pub fn gtk_window(&self) -> &ApplicationWindow {
        &self.window
    }

    /// Loads the PNG at `path`, resizes the window to its native size, and
    /// redraws. Panics if the file can't be decoded — callers are expected
    /// to have already validated the path exists via `avatar::load`.
    pub fn set_texture(&self, path: &Path) {
        let file = gtk4::gio::File::for_path(path);
        let tex = gtk4::gdk::Texture::from_file(&file)
            .unwrap_or_else(|e| panic!("failed to decode {}: {e}", path.display()));
        self.window.set_default_size(tex.width(), tex.height());
        self.drawing_area.set_content_width(tex.width());
        self.drawing_area.set_content_height(tex.height());
        *self.texture.borrow_mut() = Some(tex);
        self.drawing_area.queue_draw();
    }

    /// Sets which way the sprite faces (mirrored horizontally when not
    /// facing right). No-op, no redraw, if unchanged from last call.
    pub fn set_facing_right(&self, facing_right: bool) {
        if self.facing_right.get() != facing_right {
            self.facing_right.set(facing_right);
            self.drawing_area.queue_draw();
        }
    }

    /// Moves the window's top-left corner to `(x, y)` in screen coordinates,
    /// via a direct X11 `ConfigureWindow` request (X11-only for this slice,
    /// same rationale as `set_always_on_top`: GTK4 top-level windows have no
    /// portable positioning API, and none is needed for Wayland here). A
    /// no-op if `(x, y)` matches the last successfully-requested position,
    /// since this is called every animation tick even while the sprite is
    /// stationary (idle, landed).
    pub fn move_to(&self, x: i32, y: i32) {
        if self.last_pos.get() == Some((x, y)) {
            return;
        }
        let Some(state) = ensure_x11(&self.window, &self.x11) else {
            return;
        };
        let aux = x11rb::protocol::xproto::ConfigureWindowAux::new().x(x).y(y);
        let cookie = match state.conn.configure_window(state.xid, &aux) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("puck: failed to move window: {e:?}");
                return;
            }
        };
        if let Err(e) = cookie.check() {
            eprintln!("puck: move rejected: {e:?}");
        }
        self.last_pos.set(Some((x, y)));
    }

    /// Attaches a drag gesture to the drawing area. `on_begin` fires on
    /// press; `on_update` fires with the drag's offset from its own start
    /// point (GTK's `GestureDrag` semantics — not absolute or widget-local
    /// coordinates), so callers add it to whatever position the drag
    /// started from, not the offset itself. `on_end` fires on release.
    pub fn connect_drag<FBegin, FUpdate, FEnd>(&self, on_begin: FBegin, on_update: FUpdate, on_end: FEnd)
    where
        FBegin: Fn() + 'static,
        FUpdate: Fn(f64, f64) + 'static,
        FEnd: Fn() + 'static,
    {
        let gesture = gtk4::GestureDrag::new();
        gesture.connect_drag_begin(move |_g, _x, _y| {
            on_begin();
        });
        gesture.connect_drag_update(move |_g, offset_x, offset_y| {
            on_update(offset_x, offset_y);
        });
        gesture.connect_drag_end(move |_g, _x, _y| {
            on_end();
        });
        self.drawing_area.add_controller(gesture);
    }
}

/// Establishes the shared X11 connection on first use and caches it in
/// `cache` for subsequent calls. Returns `None` (logging nothing itself —
/// callers report failure in their own context) if the window has no
/// surface yet or isn't running on X11.
fn ensure_x11<'a>(
    window: &ApplicationWindow,
    cache: &'a RefCell<Option<X11State>>,
) -> Option<std::cell::Ref<'a, X11State>> {
    if cache.borrow().is_none() {
        let surface = window.surface()?;
        let x11_surface = surface.downcast::<X11Surface>().ok()?;
        let xid = x11_surface.xid() as u32;
        let (conn, screen_num) = RustConnection::connect(None).ok()?;
        let root = conn.setup().roots[screen_num].root;
        *cache.borrow_mut() = Some(X11State { conn, xid, root });
    }
    Some(std::cell::Ref::map(cache.borrow(), |o| {
        o.as_ref().expect("just ensured Some above")
    }))
}

/// Sets the EWMH `_NET_WM_STATE_ABOVE` hint on the window's underlying X11
/// surface. GTK4 dropped `Window::set_keep_above`, so this talks to the X
/// server directly. No-op (logs a warning) if the surface isn't X11 — e.g.
/// running under Wayland, which is out of scope for this slice.
fn set_always_on_top(window: &ApplicationWindow, cache: &RefCell<Option<X11State>>) {
    let Some(state) = ensure_x11(window, cache) else {
        eprintln!("puck: could not establish an X11 connection for the always-on-top hint");
        return;
    };

    let wm_state = intern_atom(&state.conn, "_NET_WM_STATE");
    let wm_state_above = intern_atom(&state.conn, "_NET_WM_STATE_ABOVE");
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
    let event = x11rb::protocol::xproto::ClientMessageEvent::new(32, state.xid, wm_state, data);
    let send_result = state.conn.send_event(
        false,
        state.root,
        x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_REDIRECT
            | x11rb::protocol::xproto::EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    );
    match send_result {
        Ok(cookie) => {
            if let Err(e) = cookie.check() {
                eprintln!("puck: always-on-top state change was rejected: {e:?}");
            }
        }
        Err(e) => eprintln!("puck: failed to send always-on-top state change: {e:?}"),
    }
}

fn intern_atom(conn: &RustConnection, name: &str) -> Option<u32> {
    conn.intern_atom(false, name.as_bytes())
        .ok()?
        .reply()
        .ok()
        .map(|r| r.atom)
}
