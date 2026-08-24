use super::client::{ClientError, ContentBlock, MessagesApi, ToolDefinition};
use super::session::Session;
use super::tools::{ToolHandler, ToolOutcome};

const SYSTEM_PROMPT: &str =
    "You are Puck, a helpful desktop assistant running on the user's Linux machine.";

/// A hard ceiling on how many tool-use round trips one `run_turn` call will
/// make before giving up, so a runaway loop (model bug, adversarial input)
/// can't run forever.
const MAX_TOOL_ITERATIONS: usize = 25;

/// Approves or declines a pending tool call before it runs. Implementations
/// decide how (a terminal prompt, a GUI dialog, always-allow for tests) —
/// `run_turn` never executes a tool without going through this first.
pub trait Approver {
    fn approve(&mut self, tool_name: &str, input: &serde_json::Value) -> bool;
}

/// Runs one user turn to completion: sends the conversation to the API,
/// prints any text Claude returns via `on_text`, and if Claude asks to use
/// a tool, gates each call through `approver` before running it and feeds
/// the results back — repeating until Claude stops calling tools (or the
/// iteration cap trips).
pub fn run_turn(
    api: &dyn MessagesApi,
    session: &mut Session,
    tools: &[Box<dyn ToolHandler>],
    approver: &mut dyn Approver,
    mut on_text: impl FnMut(&str),
) -> Result<(), ClientError> {
    let tool_defs: Vec<ToolDefinition> = tools.iter().map(|t| t.definition()).collect();

    for _ in 0..MAX_TOOL_ITERATIONS {
        let response = api.send(session.messages(), &tool_defs, Some(SYSTEM_PROMPT))?;

        let mut tool_uses: Vec<(String, String, serde_json::Value)> = Vec::new();
        for block in &response.content {
            match block {
                ContentBlock::Text { text } => on_text(text),
                ContentBlock::ToolUse { id, name, input } => {
                    tool_uses.push((id.clone(), name.clone(), input.clone()));
                }
                ContentBlock::ToolResult { .. } => {}
            }
        }

        session.push_assistant(response.content);

        if tool_uses.is_empty() {
            return Ok(());
        }

        let mut results = Vec::with_capacity(tool_uses.len());
        for (id, name, input) in tool_uses {
            let outcome = if !approver.approve(&name, &input) {
                ToolOutcome {
                    content: "the user declined to run this tool call".to_string(),
                    is_error: true,
                }
            } else if let Some(handler) = tools.iter().find(|t| t.definition().name == name) {
                handler.run(&input)
            } else {
                ToolOutcome {
                    content: format!("no such tool: {name}"),
                    is_error: true,
                }
            };
            results.push(ContentBlock::ToolResult {
                tool_use_id: id,
                content: outcome.content,
                is_error: if outcome.is_error { Some(true) } else { None },
            });
        }
        session.push_tool_results(results);
    }

    Err(ClientError::ToolLoopLimit(MAX_TOOL_ITERATIONS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::client::{Message, MessagesResponse, Usage};
    use std::cell::{Cell, RefCell};

    struct FakeApi {
        responses: RefCell<std::vec::IntoIter<MessagesResponse>>,
    }

    impl FakeApi {
        fn new(responses: Vec<MessagesResponse>) -> Self {
            Self {
                responses: RefCell::new(responses.into_iter()),
            }
        }
    }

    impl MessagesApi for FakeApi {
        fn send(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
            _system: Option<&str>,
        ) -> Result<MessagesResponse, ClientError> {
            Ok(self
                .responses
                .borrow_mut()
                .next()
                .expect("FakeApi ran out of canned responses"))
        }
    }

    fn text_response(text: &str) -> MessagesResponse {
        MessagesResponse {
            id: "msg_test".to_string(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            stop_reason: Some("end_turn".to_string()),
            usage: Usage {
                input_tokens: 1,
                output_tokens: 1,
            },
        }
    }

    fn tool_use_response(id: &str, name: &str, input: serde_json::Value) -> MessagesResponse {
        MessagesResponse {
            id: "msg_test".to_string(),
            role: "assistant".to_string(),
            content: vec![ContentBlock::ToolUse {
                id: id.to_string(),
                name: name.to_string(),
                input,
            }],
            stop_reason: Some("tool_use".to_string()),
            usage: Usage {
                input_tokens: 1,
                output_tokens: 1,
            },
        }
    }

    struct FixedApprover(bool);
    impl Approver for FixedApprover {
        fn approve(&mut self, _tool_name: &str, _input: &serde_json::Value) -> bool {
            self.0
        }
    }

    struct RecordingTool {
        called: Cell<bool>,
    }
    impl ToolHandler for RecordingTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "recording".to_string(),
                description: "test tool".to_string(),
                input_schema: serde_json::json!({"type": "object"}),
            }
        }
        fn run(&self, _input: &serde_json::Value) -> ToolOutcome {
            self.called.set(true);
            ToolOutcome {
                content: "ran".to_string(),
                is_error: false,
            }
        }
    }

    #[test]
    fn text_only_response_ends_the_turn_immediately() {
        let api = FakeApi::new(vec![text_response("hello there")]);
        let mut session = Session::new();
        session.push_user_text("hi");
        let tools: Vec<Box<dyn ToolHandler>> = vec![];
        let mut approver = FixedApprover(true);

        let mut seen = Vec::new();
        run_turn(&api, &mut session, &tools, &mut approver, |t| {
            seen.push(t.to_string())
        })
        .unwrap();

        assert_eq!(seen, vec!["hello there".to_string()]);
        assert_eq!(session.messages().len(), 2); // user + assistant
    }

    #[test]
    fn approved_tool_call_runs_and_feeds_the_result_back() {
        let api = FakeApi::new(vec![
            tool_use_response("toolu_1", "recording", serde_json::json!({})),
            text_response("done"),
        ]);
        let mut session = Session::new();
        session.push_user_text("do it");
        let tool = RecordingTool {
            called: Cell::new(false),
        };
        let tools: Vec<Box<dyn ToolHandler>> = vec![Box::new(tool)];
        let mut approver = FixedApprover(true);

        run_turn(&api, &mut session, &tools, &mut approver, |_| {}).unwrap();

        let msgs = session.messages();
        assert_eq!(msgs.len(), 4); // user, assistant(tool_use), user(tool_result), assistant(text)
        let ContentBlock::ToolResult {
            content, is_error, ..
        } = &msgs[2].content[0]
        else {
            panic!("expected a tool_result block");
        };
        assert_eq!(content, "ran");
        assert_eq!(*is_error, None);
    }

    #[test]
    fn declined_tool_call_never_runs_and_reports_the_decline() {
        let api = FakeApi::new(vec![
            tool_use_response("toolu_1", "recording", serde_json::json!({})),
            text_response("ok, skipping"),
        ]);
        let mut session = Session::new();
        session.push_user_text("do it");
        let tool = RecordingTool {
            called: Cell::new(false),
        };
        let tools: Vec<Box<dyn ToolHandler>> = vec![Box::new(tool)];
        let mut approver = FixedApprover(false);

        run_turn(&api, &mut session, &tools, &mut approver, |_| {}).unwrap();

        let msgs = session.messages();
        let ContentBlock::ToolResult {
            content, is_error, ..
        } = &msgs[2].content[0]
        else {
            panic!("expected a tool_result block");
        };
        assert!(content.contains("declined"));
        assert_eq!(*is_error, Some(true));
    }

    #[test]
    fn stops_after_the_iteration_cap_instead_of_looping_forever() {
        // One more canned tool_use response than the cap allows, so if the
        // loop failed to stop it would panic on FakeApi running dry rather
        // than hang.
        let responses: Vec<MessagesResponse> = (0..MAX_TOOL_ITERATIONS + 1)
            .map(|i| tool_use_response(&format!("toolu_{i}"), "recording", serde_json::json!({})))
            .collect();
        let api = FakeApi::new(responses);
        let mut session = Session::new();
        session.push_user_text("loop forever");
        let tool = RecordingTool {
            called: Cell::new(false),
        };
        let tools: Vec<Box<dyn ToolHandler>> = vec![Box::new(tool)];
        let mut approver = FixedApprover(true);

        let result = run_turn(&api, &mut session, &tools, &mut approver, |_| {});
        assert!(result.is_err());
    }
}
