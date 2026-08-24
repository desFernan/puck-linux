mod avatar;
mod motion;
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
        let win = std::rc::Rc::new(window::PuckWindow::new(app));
        let idle_path = loaded
            .clips
            .get("idle")
            .expect("idle validated by avatar::load")
            .clone();
        let walk_path = loaded.clips.get("walk").cloned();
        win.set_texture(&idle_path);

        let screen_width = gtk4::prelude::WidgetExt::display(win.gtk_window())
            .monitors()
            .item(0)
            .and_then(|m| m.downcast::<gtk4::gdk::Monitor>().ok())
            .map(|m| m.geometry().width() as f64)
            .unwrap_or(1920.0);

        let motion = std::rc::Rc::new(std::cell::RefCell::new(motion::Motion::new(
            loaded.hitbox.width,
            screen_width,
        )));
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

        // Leak the window so it isn't dropped when `connect_activate` returns;
        // the tick-loop closure above holds its own Rc clone keeping it alive.
        std::mem::forget(win);
    });

    // Pass no args to GApplication itself: we've already parsed our own
    // positional `avatar_path` argument above. If GApplication sees it too,
    // its default (non-HANDLES_COMMAND_LINE) local_command_line handling
    // treats a bare positional argument as a file to open and emits `open`
    // instead of `activate` — so the window would never appear.
    app.run_with_args(&[] as &[&str]);
}
