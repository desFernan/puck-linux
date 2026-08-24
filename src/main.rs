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
        let idle_path = loaded
            .clips
            .get("idle")
            .expect("idle validated by avatar::load");
        win.set_texture(idle_path);
        // Leak the window so it isn't dropped when `connect_activate` returns;
        // Task 4 replaces this with proper ownership via the motion loop.
        std::mem::forget(win);
    });

    // Pass no args to GApplication itself: we've already parsed our own
    // positional `avatar_path` argument above. If GApplication sees it too,
    // its default (non-HANDLES_COMMAND_LINE) local_command_line handling
    // treats a bare positional argument as a file to open and emits `open`
    // instead of `activate` — so the window would never appear.
    app.run_with_args(&[] as &[&str]);
}
