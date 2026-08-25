use puck_linux::agent::{load_dotenv, Approver, Client, RunShell, Session, ToolHandler};
use std::io::{self, BufRead, Write};

const DEFAULT_MODEL: &str = "claude-opus-5";

struct TerminalApprover;

impl Approver for TerminalApprover {
    fn approve(&mut self, tool_name: &str, input: &serde_json::Value) -> bool {
        println!("puck wants to run '{tool_name}' with input: {input}");
        print!("Allow? [y/N] ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line).unwrap_or(0) == 0 {
            return false;
        }
        matches!(line.trim().to_lowercase().as_str(), "y" | "yes")
    }
}

fn main() {
    load_dotenv(std::path::Path::new(".env"));

    let api_key = match std::env::var("ANTHROPIC_API_KEY") {
        Ok(k) if !k.is_empty() => k,
        _ => {
            eprintln!(
                "puck-agent: set ANTHROPIC_API_KEY (env var or .env in the current directory)"
            );
            std::process::exit(1);
        }
    };
    let model = std::env::var("PUCK_AGENT_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string());

    let client = Client::new(api_key, model);
    let mut session = Session::new();
    let tools: Vec<Box<dyn ToolHandler>> = vec![Box::new(RunShell)];
    let mut approver = TerminalApprover;

    println!("puck-agent — type a message, Ctrl+D to exit");
    let stdin = io::stdin();
    loop {
        print!("> ");
        let _ = io::stdout().flush();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).unwrap_or(0) == 0 {
            println!();
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        session.push_user_text(line);
        puck_linux::bridge::notify_emotion("thinking");
        let result =
            puck_linux::agent::run_turn(&client, &mut session, &tools, &mut approver, |text| {
                println!("{text}");
            });
        match &result {
            Ok(()) => puck_linux::bridge::notify_emotion("happy"),
            Err(_) => puck_linux::bridge::notify_emotion("sad"),
        }
        if let Err(e) = result {
            eprintln!("puck-agent: {e}");
        }
    }
}
