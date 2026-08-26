use super::client::ToolDefinition;
use std::io::Read;
use std::time::{Duration, Instant};

/// How long `RunShell` lets a command run before killing it. Without this,
/// a command that never exits (waiting on stdin, a stuck network call, an
/// infinite loop) would hang the agent's worker thread — and with it, the
/// whole conversation, since `turn::run_turn` waits for the tool result
/// before it can send anything else — forever, with no way to recover
/// short of restarting the process.
const SHELL_TIMEOUT: Duration = Duration::from_secs(30);

/// Caps how much of a command's output is fed back to the model. An
/// unbounded command (`cat` on a huge file, a noisy build log) would
/// otherwise blow up the conversation's token usage and cost on every
/// subsequent turn, since the full history is resent each time.
const MAX_OUTPUT_CHARS: usize = 6000;

pub struct ToolOutcome {
    pub content: String,
    pub is_error: bool,
}

/// A tool the agent can call. Every call goes through the approval gate in
/// `turn::run_turn` before `run` executes — a handler is never responsible
/// for its own approval. `Send` so a GUI client can own its tool set on a
/// background worker thread (see `puck-client`).
pub trait ToolHandler: Send {
    fn definition(&self) -> ToolDefinition;
    fn run(&self, input: &serde_json::Value) -> ToolOutcome;
}

/// Runs a shell command via `sh -c` and returns combined stdout+stderr
/// (each capped to `MAX_OUTPUT_CHARS`, combined length before appending).
///
/// `command` is untrusted model output. This tool deliberately does not
/// sandbox or allowlist commands — it runs with the same permissions as
/// this process, matching puck-mac's `run_shell` (broad machine control by
/// design). The only safety gate is the mandatory per-call user approval
/// in `turn::run_turn`, which shows the exact command before it runs.
pub struct RunShell;

impl ToolHandler for RunShell {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "run_shell".to_string(),
            description: "Runs a shell command on the user's Linux machine via `sh -c` and \
                returns its combined stdout and stderr. Call this when the user asks you to \
                run a command, inspect system or file state, or perform an operation that \
                needs the shell. The user must approve every call before it runs. Commands \
                that run longer than 30 seconds are killed."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to run, passed to `sh -c`"
                    }
                },
                "required": ["command"],
                "additionalProperties": false
            }),
        }
    }

    fn run(&self, input: &serde_json::Value) -> ToolOutcome {
        let Some(command) = input.get("command").and_then(|v| v.as_str()) else {
            return ToolOutcome {
                content: "missing required 'command' string input".to_string(),
                is_error: true,
            };
        };
        run_shell_command(command, SHELL_TIMEOUT)
    }
}

/// Truncates `output`'s combined stdout+stderr to `MAX_OUTPUT_CHARS`
/// characters, noting how much was cut. Split out from `run_shell_command`
/// so tests can check truncation without spawning a process that produces
/// megabytes of output.
fn truncate_output(output: String) -> String {
    let total = output.chars().count();
    if total <= MAX_OUTPUT_CHARS {
        return output;
    }
    let kept: String = output.chars().take(MAX_OUTPUT_CHARS).collect();
    format!(
        "{kept}\n... (truncated: {total} characters total, showing the first {MAX_OUTPUT_CHARS})"
    )
}

/// Runs `command` via `sh -c`, killing it if it's still running after
/// `timeout`. Reads stdout/stderr on their own threads concurrently with
/// waiting on the child — reading them sequentially after the child exits
/// would deadlock if the child fills a pipe's buffer before exiting (the
/// child blocks writing, we'd be blocked waiting for it to exit first).
fn run_shell_command(command: &str, timeout: Duration) -> ToolOutcome {
    let mut child = match std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            return ToolOutcome {
                content: format!("failed to run shell: {e}"),
                is_error: true,
            }
        }
    };

    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut stderr = child.stderr.take().expect("stderr was piped");
    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = stderr.read_to_end(&mut buf);
        buf
    });

    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(_) => break None,
        }
    };

    let stdout_buf = stdout_handle.join().unwrap_or_default();
    let stderr_buf = stderr_handle.join().unwrap_or_default();
    let mut combined = String::from_utf8_lossy(&stdout_buf).into_owned();
    combined.push_str(&String::from_utf8_lossy(&stderr_buf));
    let combined = truncate_output(combined);

    match status {
        Some(status) if status.success() => ToolOutcome {
            content: combined,
            is_error: false,
        },
        Some(status) => ToolOutcome {
            content: format!("command exited with status {:?}\n{combined}", status.code()),
            is_error: true,
        },
        None => ToolOutcome {
            content: format!(
                "command timed out after {}s and was killed\n{combined}",
                timeout.as_secs()
            ),
            is_error: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_shell_returns_stdout_on_success() {
        let outcome = RunShell.run(&serde_json::json!({"command": "echo hello"}));
        assert!(!outcome.is_error);
        assert_eq!(outcome.content.trim(), "hello");
    }

    #[test]
    fn run_shell_reports_failure_exit_status() {
        let outcome = RunShell.run(&serde_json::json!({"command": "exit 3"}));
        assert!(outcome.is_error);
        assert!(outcome.content.contains("3"));
    }

    #[test]
    fn run_shell_rejects_missing_command_input() {
        let outcome = RunShell.run(&serde_json::json!({}));
        assert!(outcome.is_error);
        assert!(outcome.content.contains("command"));
    }

    #[test]
    fn definition_has_a_required_command_property() {
        let def = RunShell.definition();
        assert_eq!(def.name, "run_shell");
        assert_eq!(def.input_schema["required"], serde_json::json!(["command"]));
    }

    #[test]
    fn kills_a_command_that_outlives_the_timeout() {
        let outcome = run_shell_command("sleep 5", Duration::from_millis(100));
        assert!(outcome.is_error);
        assert!(outcome.content.contains("timed out"));
    }

    #[test]
    fn a_command_finishing_before_the_timeout_is_unaffected() {
        let outcome = run_shell_command("echo quick", Duration::from_secs(5));
        assert!(!outcome.is_error);
        assert_eq!(outcome.content.trim(), "quick");
    }

    #[test]
    fn truncates_output_past_the_character_cap() {
        let huge = "a".repeat(MAX_OUTPUT_CHARS + 500);
        let truncated = truncate_output(huge);
        assert!(truncated.len() < MAX_OUTPUT_CHARS + 500);
        assert!(truncated.contains("truncated"));
    }

    #[test]
    fn does_not_touch_output_under_the_cap() {
        let short = "hello".to_string();
        assert_eq!(truncate_output(short.clone()), short);
    }
}
