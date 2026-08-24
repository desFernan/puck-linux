use gdk4_x11::X11Surface;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, DrawingArea};
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::ConnectionExt;
use x11rb::rust_connection::RustConnection;

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

        let drawing_area = DrawingArea::new();
        window.set_child(Some(&drawing_area));

        let texture: Rc<RefCell<Option<gtk4::gdk::Texture>>> = Rc::new(RefCell::new(None));
        let texture_for_draw = texture.clone();
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
            if let Err(e) = ctx.set_source_surface(&surface, 0.0, 0.0) {
                eprintln!("puck: failed to set texture as paint source: {e}");
                return;
            }
            let _ = ctx.paint();
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
        window.connect_map(|w| {
            let w = w
                .clone()
                .downcast::<ApplicationWindow>()
                .expect("map signal fired on an ApplicationWindow");
            set_always_on_top(&w);
        });

        window.present();

        let window_for_timeout = window.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(200), move || {
            set_always_on_top(&window_for_timeout);
        });

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
        let file = gtk4::gio::File::for_path(path);
        let tex = gtk4::gdk::Texture::from_file(&file)
            .unwrap_or_else(|e| panic!("failed to decode {}: {e}", path.display()));
        self.window.set_default_size(tex.width(), tex.height());
        self.drawing_area.set_content_width(tex.width());
        self.drawing_area.set_content_height(tex.height());
        *self.texture.borrow_mut() = Some(tex);
        self.drawing_area.queue_draw();
    }

    /// Moves the window's top-left corner to `(x, y)` in screen coordinates,
    /// via a direct X11 `ConfigureWindow` request (X11-only for this slice,
    /// same rationale as `set_always_on_top`: GTK4 top-level windows have no
    /// portable positioning API, and none is needed for Wayland here).
    pub fn move_to(&self, x: i32, y: i32) {
        let Some(surface) = self.window.surface() else {
            return;
        };
        let Ok(x11_surface) = surface.downcast::<X11Surface>() else {
            return;
        };
        let xid = x11_surface.xid() as u32;

        let Ok((conn, _screen_num)) = RustConnection::connect(None) else {
            eprintln!("puck: could not open X11 connection to move window");
            return;
        };
        let aux = x11rb::protocol::xproto::ConfigureWindowAux::new().x(x).y(y);
        let cookie = match conn.configure_window(xid, &aux) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("puck: failed to move window: {e:?}");
                return;
            }
        };
        if let Err(e) = cookie.check() {
            eprintln!("puck: move rejected: {e:?}");
        }
    }

    /// Attaches a drag gesture to the drawing area. `on_begin`/`on_update`
    /// fire with the drag's current point in the drawing area's own
    /// coordinates (so callers don't need to track the drag's start point
    /// themselves); `on_end` fires on release. Callers decide what a drag
    /// means (this module has no knowledge of `motion::Motion`).
    pub fn connect_drag<FBegin, FUpdate, FEnd>(
        &self,
        on_begin: FBegin,
        on_update: FUpdate,
        on_end: FEnd,
    ) where
        FBegin: Fn(f64, f64) + 'static,
        FUpdate: Fn(f64, f64) + 'static,
        FEnd: Fn() + 'static,
    {
        let gesture = gtk4::GestureDrag::new();
        gesture.connect_drag_begin(move |_g, x, y| {
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
    let send_result = conn.send_event(
        false,
        root,
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
