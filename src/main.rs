mod avatar;
mod emotion;
mod motion;
mod window;

use gtk4::prelude::*;
use gtk4::Application;
use puck_linux::bridge::{self, BridgeMessage};

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
        let win = std::rc::Rc::new(window::PuckWindow::new(app));
        let idle_path = loaded
            .clips
            .get("idle")
            .expect("idle validated by avatar::load")
            .clone();
        win.set_display_size(loaded.hitbox.width as i32, loaded.hitbox.height as i32);
        win.set_texture(&idle_path);

        let monitor_size = gtk4::prelude::WidgetExt::display(win.gtk_window())
            .monitors()
            .item(0)
            .and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok())
            .map(|m| {
                let geometry = m.geometry();
                (geometry.width() as f64, geometry.height() as f64)
            });
        let (screen_width, screen_height) = monitor_size.unwrap_or((1920.0, 1080.0));

        let motion = std::rc::Rc::new(std::cell::RefCell::new(motion::Motion::new(
            loaded.hitbox.width,
            loaded.hitbox.height,
            screen_width,
            screen_height,
        )));
        let emotion = std::rc::Rc::new(std::cell::RefCell::new(emotion::EmotionOverride::new()));
        let last_clip: std::rc::Rc<std::cell::RefCell<String>> =
            std::rc::Rc::new(std::cell::RefCell::new("idle".to_string()));

        let win_for_tick = win.clone();
        let motion_for_tick = motion.clone();
        let emotion_for_tick = emotion.clone();
        let clips_for_tick = loaded.clips.clone();
        let idle_path_for_tick = idle_path.clone();
        gtk4::glib::source::timeout_add_local(std::time::Duration::from_millis(16), move || {
            let frame = motion_for_tick.borrow_mut().tick(0.016);
            let effective_clip = emotion_for_tick
                .borrow_mut()
                .tick()
                .map(|c| c.to_string())
                .unwrap_or_else(|| frame.clip.to_string());
            if effective_clip != *last_clip.borrow() {
                let path = clips_for_tick
                    .get(&effective_clip)
                    .unwrap_or(&idle_path_for_tick);
                win_for_tick.set_texture(path);
                *last_clip.borrow_mut() = effective_clip;
            }
            win_for_tick.set_facing_right(frame.facing_right);
            win_for_tick.move_to(frame.x as i32, frame.y as i32);
            gtk4::glib::ControlFlow::Continue
        });

        let motion_for_drag_begin = motion.clone();
        let motion_for_drag_update = motion.clone();
        let motion_for_drag_end = motion.clone();
        let win_for_drag_update = win.clone();
        win.connect_drag(
            move || motion_for_drag_begin.borrow_mut().begin_drag(),
            move |offset_x, offset_y| {
                let (x, y) = motion_for_drag_update
                    .borrow_mut()
                    .drag_to(offset_x, offset_y);
                // Move the window right away rather than waiting for the
                // next 16ms tick — GestureDrag can report updates faster
                // than that, and waiting made dragging feel laggy.
                win_for_drag_update.move_to(x as i32, y as i32);
            },
            move || motion_for_drag_end.borrow_mut().end_drag(),
        );

        spawn_bridge_listener(emotion.clone());

        // `win` itself drops here when `connect_activate` returns, but the
        // tick-loop closure above holds its own `Rc` clone (`win_for_tick`)
        // for the process's lifetime, so the window stays alive.
    });

    // Pass no args to GApplication itself: we've already parsed our own
    // positional `avatar_path` argument above. If GApplication sees it too,
    // its default (non-HANDLES_COMMAND_LINE) local_command_line handling
    // treats a bare positional argument as a file to open and emits `open`
    // instead of `activate` — so the window would never appear.
    app.run_with_args(&[] as &[&str]);
}

/// Listens on the bridge socket for `BridgeMessage`s from an agent front
/// end (`puck-agent`/`puck-client`) and applies them to `emotion`. Runs on
/// a background thread (accepting connections blocks); messages are
/// forwarded to the GTK main thread over `async_channel`, same pattern as
/// `puck-client`'s worker thread. Best-effort: if the socket can't be
/// bound (e.g. permissions, or another pet instance already holds it),
/// logs a warning and the pet runs on without bridge support rather than
/// failing to start.
fn spawn_bridge_listener(emotion: std::rc::Rc<std::cell::RefCell<emotion::EmotionOverride>>) {
    let path = bridge::socket_path();
    let listener = match bridge::listen(&path) {
        Ok(l) => l,
        Err(e) => {
            eprintln!(
                "puck: could not start bridge listener at {}: {e}",
                path.display()
            );
            return;
        }
    };

    let (tx, rx) = async_channel::unbounded::<BridgeMessage>();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            if let Ok(Some(message)) = bridge::read_message(&stream) {
                if tx.send_blocking(message).is_err() {
                    break; // main thread is gone
                }
            }
        }
    });

    gtk4::glib::spawn_future_local(async move {
        while let Ok(message) = rx.recv().await {
            let BridgeMessage::SetEmotion { clip } = message;
            emotion.borrow_mut().set(clip, emotion::EMOTION_TICKS);
        }
    });
}
