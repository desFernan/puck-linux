use super::client::{ClientError, ContentBlock, Message, MessagesApi, ToolDefinition};
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

fn text_message(role: &str, text: &str) -> Message {
    Message {
        role: role.to_string(),
        content: vec![ContentBlock::Text {
            text: text.to_string(),
        }],
    }
}

/// Runs one user turn to completion: sends `user_text` (plus the session's
/// prior history) to the API, prints any text Claude returns via
/// `on_text`, and if Claude asks to use a tool, gates each call through
/// `approver` before running it and feeds the results back — repeating
/// until Claude stops calling tools (or the iteration cap trips).
///
/// The whole turn is built on a private copy of history and written back
/// to `session` in one `Session::commit` call at the end — see that
/// method's doc for why a mid-turn failure must never partially update the
/// real session.
pub fn run_turn(
    api: &dyn MessagesApi,
    session: &mut Session,
    tools: &[Box<dyn ToolHandler>],
    approver: &mut dyn Approver,
    user_text: &str,
    mut on_text: impl FnMut(&str),
) -> Result<(), ClientError> {
    let tool_defs: Vec<ToolDefinition> = tools.iter().map(|t| t.definition()).collect();
    let original = session.messages().to_vec();
    let user_message = text_message("user", user_text);

    let mut working = original.clone();
    working.push(user_message.clone());

    for _ in 0..MAX_TOOL_ITERATIONS {
        // On failure here, `session` hasn't been touched at all yet (only
        // the local `working` copy has) — safe to retry with the same
        // user message on the next call.
        let response = api.send(&working, &tool_defs, Some(SYSTEM_PROMPT))?;

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

        working.push(Message {
            role: "assistant".to_string(),
            content: response.content,
        });

        if tool_uses.is_empty() {
            session.commit(working);
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
        working.push(Message {
            role: "user".to_string(),
            content: results,
        });
    }

    // Gave up. Commit just the original history plus this turn's question
    // and a short synthetic note — not `working`, which still holds every
    // abandoned tool round trip from this attempt. Ending on a "user" role
    // message (by committing `working` as-is, or just `original` +
    // `user_message`) would also make the *next* turn's user message land
    // right after this one with no assistant reply in between, which the
    // API rejects the same way an unpaired tool_use does.
    let mut committed = original;
    committed.push(user_message);
    committed.push(text_message(
        "assistant",
        "(gave up after reaching the tool-call limit without a final answer)",
    ));
    session.commit(committed);

    Err(ClientError::ToolLoopLimit(MAX_TOOL_ITERATIONS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::client::{MessagesResponse, Usage};
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

    /// An API double that fails outright on its `fail_on`-th call
    /// (0-indexed), so tests can verify `session` is left untouched or
    /// correctly trimmed after a mid-turn failure.
    struct FailingApi {
        responses: RefCell<std::vec::IntoIter<MessagesResponse>>,
        call_count: Cell<usize>,
        fail_on: usize,
    }

    impl MessagesApi for FailingApi {
        fn send(
            &self,
            _messages: &[Message],
            _tools: &[ToolDefinition],
            _system: Option<&str>,
        ) -> Result<MessagesResponse, ClientError> {
            let n = self.call_count.get();
            self.call_count.set(n + 1);
            if n == self.fail_on {
                return Err(ClientError::Api {
                    status: 500,
                    error_type: "overloaded_error".to_string(),
                    message: "simulated failure".to_string(),
                });
            }
            Ok(self
                .responses
                .borrow_mut()
                .next()
                .expect("FailingApi ran out of canned responses"))
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
        let tools: Vec<Box<dyn ToolHandler>> = vec![];
        let mut approver = FixedApprover(true);

        let mut seen = Vec::new();
        run_turn(&api, &mut session, &tools, &mut approver, "hi", |t| {
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
        let tool = RecordingTool {
            called: Cell::new(false),
        };
        let tools: Vec<Box<dyn ToolHandler>> = vec![Box::new(tool)];
        let mut approver = FixedApprover(true);

        run_turn(&api, &mut session, &tools, &mut approver, "do it", |_| {}).unwrap();

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
        let tool = RecordingTool {
            called: Cell::new(false),
        };
        let tools: Vec<Box<dyn ToolHandler>> = vec![Box::new(tool)];
        let mut approver = FixedApprover(false);

        run_turn(&api, &mut session, &tools, &mut approver, "do it", |_| {}).unwrap();

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
        let tool = RecordingTool {
            called: Cell::new(false),
        };
        let tools: Vec<Box<dyn ToolHandler>> = vec![Box::new(tool)];
        let mut approver = FixedApprover(true);

        let result = run_turn(
            &api,
            &mut session,
            &tools,
            &mut approver,
            "loop forever",
            |_| {},
        );
        assert!(result.is_err());
    }

    #[test]
    fn hitting_the_iteration_cap_commits_only_the_question_and_a_note_not_the_abandoned_trail() {
        let responses: Vec<MessagesResponse> = (0..MAX_TOOL_ITERATIONS + 1)
            .map(|i| tool_use_response(&format!("toolu_{i}"), "recording", serde_json::json!({})))
            .collect();
        let api = FakeApi::new(responses);
        let mut session = Session::new();
        let tool = RecordingTool {
            called: Cell::new(false),
        };
        let tools: Vec<Box<dyn ToolHandler>> = vec![Box::new(tool)];
        let mut approver = FixedApprover(true);

        let _ = run_turn(
            &api,
            &mut session,
            &tools,
            &mut approver,
            "loop forever",
            |_| {},
        );

        let msgs = session.messages();
        assert_eq!(
            msgs.len(),
            2,
            "should be exactly [user, assistant-note], not every abandoned tool round trip"
        );
        assert_eq!(msgs[0].role, "user");
        assert_eq!(msgs[1].role, "assistant");
    }

    #[test]
    fn a_network_failure_on_the_first_call_leaves_the_session_completely_untouched() {
        let api = FailingApi {
            responses: RefCell::new(Vec::new().into_iter()),
            call_count: Cell::new(0),
            fail_on: 0, // fail immediately, before any progress
        };
        let mut session = Session::new();
        let tools: Vec<Box<dyn ToolHandler>> = vec![];
        let mut approver = FixedApprover(true);

        let result = run_turn(&api, &mut session, &tools, &mut approver, "hi", |_| {});

        assert!(result.is_err());
        assert!(
            session.messages().is_empty(),
            "a failed first call must not leave a lone unanswered user message behind - \
             the next successful turn would otherwise push a second consecutive user message"
        );
    }

    #[test]
    fn a_network_failure_mid_turn_leaves_the_previous_turns_history_intact() {
        // First turn succeeds normally.
        let api = FakeApi::new(vec![text_response("hello")]);
        let mut session = Session::new();
        let tools: Vec<Box<dyn ToolHandler>> = vec![];
        let mut approver = FixedApprover(true);
        run_turn(&api, &mut session, &tools, &mut approver, "hi", |_| {}).unwrap();
        assert_eq!(session.messages().len(), 2);

        // Second turn's very first API call fails.
        let failing = FailingApi {
            responses: RefCell::new(Vec::new().into_iter()),
            call_count: Cell::new(0),
            fail_on: 0,
        };
        let result = run_turn(
            &failing,
            &mut session,
            &tools,
            &mut approver,
            "how are you",
            |_| {},
        );

        assert!(result.is_err());
        assert_eq!(
            session.messages().len(),
            2,
            "the first turn's history must survive a second turn's failed API call untouched"
        );
    }
}
