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

    // Pass no args to GApplication itself: we've already parsed our own
    // positional `avatar_path` argument above. If GApplication sees it too,
    // its default (non-HANDLES_COMMAND_LINE) local_command_line handling
    // treats a bare positional argument as a file to open and emits `open`
    // instead of `activate` — so the window would never appear.
    app.run_with_args(&[] as &[&str]);
}
