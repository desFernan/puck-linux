use super::client::ToolDefinition;

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

/// Runs a shell command via `sh -c` and returns combined stdout+stderr.
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
                needs the shell. The user must approve every call before it runs."
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

        match std::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
        {
            Ok(output) => {
                let mut combined = String::from_utf8_lossy(&output.stdout).into_owned();
                combined.push_str(&String::from_utf8_lossy(&output.stderr));
                if output.status.success() {
                    ToolOutcome {
                        content: combined,
                        is_error: false,
                    }
                } else {
                    ToolOutcome {
                        content: format!(
                            "command exited with status {:?}\n{combined}",
                            output.status.code()
                        ),
                        is_error: true,
                    }
                }
            }
            Err(e) => ToolOutcome {
                content: format!("failed to run shell: {e}"),
                is_error: true,
            },
        }
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
}
