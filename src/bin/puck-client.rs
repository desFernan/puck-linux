use gtk4::prelude::*;
use gtk4::{glib, Application, ApplicationWindow};
use puck_linux::agent::{load_dotenv, run_turn, Approver, Client, RunShell, Session, ToolHandler};
use std::cell::RefCell;
use std::sync::mpsc;

const DEFAULT_MODEL: &str = "claude-opus-5";
const APP_ID: &str = "dev.desfernan.PuckClient";

/// Messages the background agent-worker thread sends to the GTK main
/// thread. The worker owns the `Session`/`Client`/tools and runs
/// `run_turn` synchronously (it's a blocking HTTP call); this channel is
/// how its output reaches the UI without blocking it.
enum UiEvent {
    AssistantText(String),
    Error(String),
    ApprovalRequest {
        tool_name: String,
        input: serde_json::Value,
        reply: mpsc::Sender<bool>,
    },
}

/// Commands the UI thread sends to the worker.
enum WorkerCommand {
    UserMessage(String),
}

/// Approves tool calls by asking the GTK main thread to show a dialog and
/// blocking (on this, the worker thread — never the UI thread) for its
/// answer. `run_turn` calls this synchronously from the worker thread.
struct GtkApprover {
    ui_tx: async_channel::Sender<UiEvent>,
}

impl Approver for GtkApprover {
    fn approve(&mut self, tool_name: &str, input: &serde_json::Value) -> bool {
        let (reply_tx, reply_rx) = mpsc::channel();
        let sent = self.ui_tx.send_blocking(UiEvent::ApprovalRequest {
            tool_name: tool_name.to_string(),
            input: input.clone(),
            reply: reply_tx,
        });
        if sent.is_err() {
            return false; // UI thread is gone; fail closed.
        }
        reply_rx.recv().unwrap_or(false)
    }
}

/// Starts the long-lived agent worker thread. It owns the conversation
/// `Session` for the process's lifetime (so history persists across
/// messages) and processes one `WorkerCommand` at a time from `cmd_rx`,
/// reporting text and approval requests back over `ui_tx`.
fn spawn_worker(
    client: Client,
    tools: Vec<Box<dyn ToolHandler>>,
    ui_tx: async_channel::Sender<UiEvent>,
) -> mpsc::Sender<WorkerCommand> {
    let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCommand>();
    std::thread::spawn(move || {
        let mut session = Session::new();
        for command in cmd_rx {
            let WorkerCommand::UserMessage(text) = command;

            let mut approver = GtkApprover {
                ui_tx: ui_tx.clone(),
            };
            let text_tx = ui_tx.clone();
            puck_linux::bridge::notify_emotion("thinking");
            let result = run_turn(&client, &mut session, &tools, &mut approver, &text, |t| {
                let _ = text_tx.send_blocking(UiEvent::AssistantText(t.to_string()));
            });
            match &result {
                Ok(()) => puck_linux::bridge::notify_emotion("happy"),
                Err(_) => puck_linux::bridge::notify_emotion("sad"),
            }
            if let Err(e) = result {
                let _ = ui_tx.send_blocking(UiEvent::Error(e.to_string()));
            }
        }
    });
    cmd_tx
}

fn append_line(buffer: &gtk4::TextBuffer, text: &str) {
    let mut end = buffer.end_iter();
    buffer.insert(&mut end, text);
    buffer.insert(&mut end, "\n");
}

fn build_ui(app: &Application, client: Client) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Puck")
        .default_width(480)
        .default_height(640)
        .build();

    let transcript_view = gtk4::TextView::builder()
        .editable(false)
        .cursor_visible(false)
        .wrap_mode(gtk4::WrapMode::WordChar)
        .build();
    let transcript_buffer = transcript_view.buffer();
    append_line(
        &transcript_buffer,
        "puck-client — type a message below and press Enter.",
    );

    let scroller = gtk4::ScrolledWindow::builder()
        .child(&transcript_view)
        .vexpand(true)
        .build();

    let entry = gtk4::Entry::builder()
        .placeholder_text("Message Puck...")
        .build();

    let layout = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
    layout.append(&scroller);
    layout.append(&entry);
    window.set_child(Some(&layout));
    window.present();

    // glib dropped its synchronous MainContext channel; the current
    // gtk4-rs pattern is a plain async channel drained by a future running
    // on the main thread's context via spawn_future_local (no `Send`
    // requirement, so the loop body below can freely touch GTK widgets).
    let (ui_tx, ui_rx) = async_channel::unbounded::<UiEvent>();
    let tools: Vec<Box<dyn ToolHandler>> = vec![Box::new(RunShell)];
    let cmd_tx = spawn_worker(client, tools, ui_tx);

    let window_for_events = window.clone();
    let buffer_for_events = transcript_buffer.clone();
    glib::spawn_future_local(async move {
        while let Ok(event) = ui_rx.recv().await {
            match event {
                UiEvent::AssistantText(text) => append_line(&buffer_for_events, &text),
                UiEvent::Error(err) => append_line(&buffer_for_events, &format!("[error] {err}")),
                UiEvent::ApprovalRequest {
                    tool_name,
                    input,
                    reply,
                } => {
                    let pending_reply = RefCell::new(Some(reply));
                    let dialog = gtk4::MessageDialog::builder()
                        .transient_for(&window_for_events)
                        .modal(true)
                        .message_type(gtk4::MessageType::Question)
                        .buttons(gtk4::ButtonsType::YesNo)
                        .text(format!("Allow tool '{tool_name}'?"))
                        .secondary_text(input.to_string())
                        .build();
                    dialog.connect_response(move |dlg, response| {
                        if let Some(reply) = pending_reply.borrow_mut().take() {
                            let _ = reply.send(response == gtk4::ResponseType::Yes);
                        }
                        dlg.close();
                    });
                    dialog.show();
                }
            }
        }
    });

    entry.connect_activate(move |entry| {
        let text = entry.text().to_string();
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        append_line(&transcript_buffer, &format!("> {text}"));
        let _ = cmd_tx.send(WorkerCommand::UserMessage(text.to_string()));
        entry.set_text("");
    });
}

fn main() {
    load_dotenv(std::path::Path::new(".env"));

    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!(
                "puck-client: set ANTHROPIC_API_KEY (env var or .env in the current directory)"
            );
            std::process::exit(1);
        }
    };
    let model = std::env::var("PUCK_AGENT_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());
    let client = Client::new(api_key, model);

    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(move |app| {
        build_ui(app, client.clone());
    });
    app.run_with_args(&[] as &[&str]);
}
